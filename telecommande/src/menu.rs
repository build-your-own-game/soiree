//! Le menu de la console, dans le navigateur : le même pour tous les jeux.
//! Il démarre avec le lien de la page ([`crate::web::connect`]) : le jeu n'a
//! rien à écrire, et n'a pas à savoir se quitter.
//!
//! - Sur l'ordinateur de l'hôte, Échap maintenu ouvre le menu : le jeu se
//!   fige pour tout le monde. Un appui court est rendu au jeu, qui en fait ce
//!   qu'il veut : sa propre pause ([`crate::web::pause`]), par exemple.
//! - Sur un téléphone, la page se couvre d'un voile pendant la pause, et
//!   attend le jeu suivant quand l'hôte ferme celui-ci.
//!
//! La console dit quoi montrer par le lien `/soiree/console` : d'abord `ici`
//! (cette page est sur l'ordinateur de l'hôte), puis l'état du menu.

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::rc::Rc;

use serde::Deserialize;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::js_sys::{Array, Function, Object, Proxy, Reflect};
use web_sys::{
    AddEventListenerOptions, CloseEvent, Document, Element, Event, EventTarget, HtmlElement,
    KeyboardEvent, KeyboardEventInit, MessageEvent, ShadowRoot, ShadowRootInit, ShadowRootMode,
    TouchEvent, WebSocket, Window,
};

/// Le temps qu'Échap doit rester enfoncé pour ouvrir le menu.
const HOLD_MS: i32 = 600;
const HINT_MS: i32 = 2600;
const ESCAPE_CODE: u32 = 27;
const RETRY_MS: i32 = 1000;
/// Sans réponse du jeu autant de fois de suite, l'écran retourne à l'étagère.
const LOST_TRIES: u32 = 3;
const ESCAPE: &str = "Escape";

/// Ce que la console règle dans toute page, écran ou télécommande : la page
/// ne zoome pas. Sur un téléphone, deux appuis rapprochés sur un bouton, ou
/// deux pouces sur une manette, agrandissaient la page, et le joueur se
/// retrouvait devant un bout d'écran. L'app iPhone le règle de son côté
/// (`iphone/Soiree/reglages.js`) ; ici, c'est pour le navigateur.
///
/// Cette règle suffit là où le navigateur la suit. Safari, sur iPhone, ne la
/// suit pas toujours : un joueur a vu la manette de Jumpy zoomer alors qu'elle
/// dit `touch-action: none` partout (le bogue 218015 de son moteur parle des
/// éléments placés en `position: absolute`). [`no_zoom`] fait le reste à la
/// main.
const PAGE_STYLE: &str = "html { touch-action: manipulation; }";
/// Deux appuis sont un double appui s'ils se suivent d'aussi près, au même
/// endroit : c'est ce que Safari prend pour une demande de zoom.
const DOUBLE_TAP_MS: f64 = 400.0;
const DOUBLE_TAP_REACH: f64 = 48.0;
/// Un doigt qui a glissé plus loin que ça n'a pas appuyé : il a fait défiler.
const TAP_REACH: f64 = 12.0;

const STYLE: &str = "<style>
:host { all: initial; }
* { box-sizing: border-box; margin: 0; }
.veil { position: fixed; inset: 0; z-index: 2147483647; display: grid; place-items: center;
  background: rgba(8, 6, 24, .86); backdrop-filter: blur(10px); -webkit-backdrop-filter: blur(10px);
  color: #f3efff; font: 500 clamp(16px, 2.2vmin, 26px)/1.35 system-ui, -apple-system, 'Segoe UI', sans-serif;
  -webkit-user-select: none; user-select: none; touch-action: none; text-align: center; }
.panel { width: min(92vw, 24em); padding: 1.8em 1.8em 1.5em; border-radius: .9em; background: #15103a;
  box-shadow: 0 0 0 2px #3a2f86, 0 1.4em 4em rgba(0, 0, 0, .6); }
.label { font-size: .7em; letter-spacing: .22em; text-transform: uppercase; color: #3ff2ff; }
h1 { font-size: 1.9em; line-height: 1.1; margin: .15em 0 .7em; font-weight: 700; overflow-wrap: anywhere; }
p { color: #c9c0f2; margin-bottom: .6em; }
button { display: block; width: 100%; padding: .6em .8em; margin-top: .4em; border: 0; border-radius: .5em;
  cursor: pointer; font: inherit; font-weight: 600; text-align: center; color: #f3efff; background: #221a57; }
button.on { background: #ff3ea5; color: #1a0620; }
.qr { margin-top: 1.4em; padding-top: 1.2em; border-top: 2px solid #2a2168; font-size: .7em; color: #c9c0f2; }
.qr img { display: block; width: 9em; height: 9em; padding: .5em; margin: 0 auto .6em; background: #fff; border-radius: .5em; }
.delays { margin-top: 1.2em; font-size: .7em; color: #c9c0f2; }
.delays b { font-weight: 600; color: #ffcf5c; }
.keys { margin-top: 1.2em; font-size: .68em; color: #8f85c9; }
.lone { padding: 2em; }
.lone h1 { margin-bottom: .3em; }
.lone p { margin: 0; }
.pill { position: fixed; left: 50%; bottom: 4vh; transform: translateX(-50%); z-index: 2147483647;
  padding: .6em 1em; border-radius: 99em; background: rgba(8, 6, 24, .92); color: #f3efff; text-align: center;
  box-shadow: 0 0 0 2px #3a2f86; font: 600 clamp(14px, 1.8vmin, 20px)/1.2 system-ui, -apple-system, 'Segoe UI', sans-serif;
  overflow: hidden; pointer-events: none; white-space: nowrap; }
.pill i { position: absolute; left: 0; bottom: 0; height: 4px; width: 100%; background: #ff3ea5;
  transform-origin: left; transform: scaleX(0); }
.pill.filling i { animation: fill .6s linear forwards; }
@keyframes fill { to { transform: scaleX(1); } }
</style>";

/// Ce que la console dit à la page (voir `Menu::state` dans `console.rs`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "le message de la console, tel qu'elle l'écrit : un champ par chose à montrer"
)]
struct State {
    /// Le premier message : cette page est-elle sur l'ordinateur de l'hôte ?
    ici: Option<bool>,
    /// Avec lui : cette page est-elle entrée dans la partie ? Faux pour un
    /// téléphone qui n'a pas scanné le QR code de cette partie.
    billet: Option<bool>,
    partie: Option<String>,
    jeu: Option<String>,
    pause: bool,
    ferme: bool,
    /// Le jeu a demandé la pause : le temps est arrêté, sans le menu.
    tenu: bool,
    recommencer: bool,
    qr: bool,
    etagere: Option<String>,
    /// Le délai de chaque télécommande, en millisecondes, du plus court au
    /// plus long. La console ne le dit qu'à l'hôte, et que menu ouvert.
    delais: Vec<u32>,
}

/// Ce que la page montre par-dessus le jeu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Showing {
    /// Le menu, sur l'ordinateur de l'hôte.
    Menu,
    /// Le voile de la pause, sur un téléphone.
    Paused,
    /// Le jeu est fermé : pour le suivant, un téléphone scannera son QR code.
    Closed,
    /// Ce téléphone n'a pas le billet de la partie : il n'y est pas entré.
    Uninvited,
    /// Le jeu a demandé la pause, sur l'ordinateur de l'hôte : l'image et le
    /// son se figent, et le jeu montre ce qu'il veut par-dessus.
    Held,
}

impl Showing {
    /// La console couvre la page : les touches ne vont plus au jeu.
    fn covers(self) -> bool {
        match self {
            Self::Menu | Self::Paused | Self::Closed | Self::Uninvited => true,
            Self::Held => false,
        }
    }
}

/// Une ligne du menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Resume,
    AskRestart,
    Quit,
    BackToMenu,
    Restart,
}

impl Item {
    fn label(self) -> &'static str {
        match self {
            Self::Resume => "Reprendre",
            Self::AskRestart => "Recommencer la partie",
            Self::Quit => "Quitter le jeu",
            Self::BackToMenu => "Non, revenir au menu",
            Self::Restart => "Oui, recommencer",
        }
    }
}

/// La question que le menu pose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Asking {
    /// Que faire ?
    #[default]
    What,
    /// Recommencer, vraiment ?
    Restart,
}

/// Le bandeau du bas de l'écran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pill {
    /// Échap est enfoncé : la barre se remplit jusqu'au menu.
    Holding,
    /// Un appui court vient d'être rendu au jeu : on dit le geste du menu.
    Hint,
}

/// L'appui sur Échap en cours.
struct Hold {
    target: Option<EventTarget>,
    /// Le délai avant le menu. `None` : il est passé, le menu est demandé.
    timer: Option<i32>,
}

/// Une page est-elle entrée dans la partie ? Sur l'ordinateur de l'hôte,
/// toujours ; ailleurs, seulement avec le billet que porte le QR code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Entry {
    #[default]
    In,
    Out,
}

#[derive(Default)]
struct Inner {
    local: bool,
    state: State,
    /// La partie que cette page a vue en premier.
    party: Option<String>,
    shelf: Option<String>,
    socket: Option<WebSocket>,
    lost: u32,
    closed: bool,
    /// Ce que la console a dit de cette page : entrée dans la partie, ou non.
    entry: Entry,
    hold: Option<Hold>,
    /// Le geste du menu a déjà été dit sur cette page.
    hinted: bool,
    pill: Option<Pill>,
    hint_timer: Option<i32>,
    choice: usize,
    asking: Asking,
    root: Option<ShadowRoot>,
}

struct Menu {
    window: Window,
    document: Document,
    inner: RefCell<Inner>,
    frames: Option<Rc<Frames>>,
    sound: Rc<Sound>,
}

/// Le son de la page : pendant la pause, la console le coupe, et le rend à
/// la reprise tel qu'il était.
///
/// Elle se glisse pour cela devant les méthodes par lesquelles une page joue
/// un son, pour connaître chaque sortie de son que le jeu utilise. Les balises
/// `<audio>` et `<video>` de la page, elle les trouve en la parcourant.
struct Sound {
    /// Les sorties de son que la page a ouvertes.
    contexts: RefCell<Vec<JsValue>>,
    /// Ce que la console a coupé, et doit relancer : `None` hors de la pause.
    cut: RefCell<Option<Cut>>,
}

/// Ce qu'une page appelle pour jouer un son : au moins l'une de ces
/// méthodes d'une sortie de son, avant le moindre bruit.
const USED_TO_PLAY: [&str; 8] = [
    "createBufferSource",
    "createGain",
    "createOscillator",
    "createMediaElementSource",
    "createMediaStreamSource",
    "createBuffer",
    "decodeAudioData",
    "resume",
];

/// Ce que la console a coupé en ouvrant le menu.
struct Cut {
    contexts: Vec<JsValue>,
    media: Vec<JsValue>,
}

/// Appelle sans argument la méthode `name` de `object`, si elle existe.
fn call(object: &JsValue, name: &str) {
    if let Some(method) = Reflect::get(object, &name.into())
        .ok()
        .and_then(|method| method.dyn_into::<Function>().ok())
    {
        let _: Result<JsValue, JsValue> = method.call0(object);
    }
}

impl Sound {
    fn install(window: &Window) -> Rc<Self> {
        let sound = Rc::new(Self {
            contexts: RefCell::new(Vec::new()),
            cut: RefCell::new(None),
        });
        // Safari a longtemps nommé le sien `webkitAudioContext`.
        let prototype = ["BaseAudioContext", "AudioContext", "webkitAudioContext"]
            .into_iter()
            .find_map(|name| {
                let class = Reflect::get(window, &name.into()).ok()?;
                Reflect::get(&class, &"prototype".into())
                    .ok()
                    .filter(JsValue::is_object)
            });
        let Some(prototype) = prototype else {
            return sound;
        };
        for name in USED_TO_PLAY {
            let Some(original) = Reflect::get(&prototype, &name.into())
                .ok()
                .and_then(|original| original.dyn_into::<Function>().ok())
            else {
                continue;
            };
            // Appelée à la place de la méthode : note la sortie de son, puis
            // laisse la méthode faire son travail.
            let apply = {
                let sound = Rc::clone(&sound);
                Closure::<dyn FnMut(JsValue, JsValue, JsValue) -> JsValue>::new(
                    move |target: JsValue, context: JsValue, arguments: JsValue| {
                        let known = sound.contexts.borrow().contains(&context);
                        if !known && context.is_object() {
                            sound.contexts.borrow_mut().push(context.clone());
                        }
                        Reflect::apply(
                            target.unchecked_ref::<Function>(),
                            &context,
                            arguments.unchecked_ref::<Array>(),
                        )
                        .unwrap_or_else(|error| wasm_bindgen::throw_val(error))
                    },
                )
            };
            let handler = Object::new();
            let trapped = Reflect::set(&handler, &"apply".into(), apply.as_ref());
            apply.forget();
            if trapped.is_ok() {
                let proxy = Proxy::new(&original, &handler);
                let _: Result<bool, JsValue> = Reflect::set(&prototype, &name.into(), &proxy);
            }
        }
        sound
    }

    /// Coupe ou rend le son de la page.
    fn hold(&self, document: &Document, showing: Option<Showing>) {
        let mut cut = self.cut.borrow_mut();
        match (showing, cut.take()) {
            (Some(_), None) => {
                let playing = |context: &&JsValue| {
                    Reflect::get(context, &"state".into())
                        .ok()
                        .and_then(|state| state.as_string())
                        .is_some_and(|state| state == "running")
                };
                let contexts: Vec<JsValue> = self
                    .contexts
                    .borrow()
                    .iter()
                    .filter(playing)
                    .cloned()
                    .collect();
                let mut media = Vec::new();
                if let Ok(found) = document.query_selector_all("audio, video") {
                    for index in 0..found.length() {
                        let Some(element) = found.item(index).map(JsValue::from) else {
                            continue;
                        };
                        let paused = Reflect::get(&element, &"paused".into())
                            .ok()
                            .and_then(|paused| paused.as_bool());
                        if paused == Some(false) {
                            media.push(element);
                        }
                    }
                }
                for context in &contexts {
                    call(context, "suspend");
                }
                for element in &media {
                    call(element, "pause");
                }
                *cut = Some(Cut { contexts, media });
            }
            (None, Some(was)) => {
                for context in &was.contexts {
                    call(context, "resume");
                }
                for element in &was.media {
                    call(element, "play");
                }
            }
            (Some(_), Some(was)) => *cut = Some(was),
            (None, None) => {}
        }
    }
}

/// Les images de la page. Entre deux vues, une page anime souvent la scène
/// toute seule (une balle en vol, un compte à rebours) : pendant la pause, la
/// console retient ses images et arrête son horloge, pour que l'écran se fige
/// avec la partie. À la reprise, l'horloge repart d'où elle était : la page
/// ne voit pas de saut.
///
/// La console remplace pour cela `requestAnimationFrame` et
/// `performance.now` de la page par les siens.
struct Frames {
    window: Window,
    performance: JsValue,
    /// Les fonctions du navigateur, telles qu'elles étaient.
    request: Function,
    now: Function,
    /// Les images demandées pendant la pause, à rendre à la reprise.
    held: RefCell<Vec<Function>>,
    /// Depuis quand l'horloge est arrêtée.
    frozen: Cell<Option<f64>>,
    /// Le temps passé en pause depuis le chargement de la page.
    paused: Cell<f64>,
}

impl Frames {
    /// Prend la place des deux fonctions du navigateur. `None` si la page n'en
    /// a pas : rien ne se fige, et le reste du menu marche quand même.
    fn install(window: &Window) -> Option<Rc<Self>> {
        let performance = Reflect::get(window, &"performance".into()).ok()?;
        let frames = Rc::new(Self {
            window: window.clone(),
            request: Reflect::get(window, &"requestAnimationFrame".into())
                .ok()?
                .dyn_into()
                .ok()?,
            now: Reflect::get(&performance, &"now".into())
                .ok()?
                .dyn_into()
                .ok()?,
            performance,
            held: RefCell::new(Vec::new()),
            frozen: Cell::new(None),
            paused: Cell::new(0.0),
        });
        let request = {
            let frames = Rc::clone(&frames);
            Closure::<dyn FnMut(Function) -> JsValue>::new(move |frame: Function| {
                if frames.frozen.get().is_some() {
                    frames.held.borrow_mut().push(frame);
                    JsValue::from(0)
                } else {
                    frames.request(frame)
                }
            })
        };
        let now = {
            let frames = Rc::clone(&frames);
            Closure::<dyn FnMut() -> f64>::new(move || frames.clock())
        };
        let replaced = Reflect::set(window, &"requestAnimationFrame".into(), request.as_ref())
            .and_then(|_| Reflect::set(&frames.performance, &"now".into(), now.as_ref()));
        request.forget();
        now.forget();
        replaced.ok().map(|_| frames)
    }

    /// L'heure du navigateur, sans rien retirer.
    fn real_now(&self) -> f64 {
        self.now
            .call0(&self.performance)
            .ok()
            .and_then(|now| now.as_f64())
            .unwrap_or_default()
    }

    /// L'heure que voit la page : celle du navigateur, moins les pauses.
    fn clock(&self) -> f64 {
        self.frozen.get().unwrap_or_else(|| self.real_now()) - self.paused.get()
    }

    /// Demande une image au navigateur, et donne à la page l'heure sans les
    /// pauses.
    fn request(&self, frame: Function) -> JsValue {
        let paused = self.paused.get();
        let asked: JsValue = if paused > 0.0 {
            Closure::once_into_js(move |time: f64| {
                let _: Result<JsValue, JsValue> =
                    frame.call1(&JsValue::NULL, &JsValue::from(time - paused));
            })
        } else {
            frame.into()
        };
        self.request
            .call1(&self.window, &asked)
            .unwrap_or(JsValue::from(0))
    }

    /// Fige ou relâche les images de la page.
    fn hold(&self, showing: Option<Showing>) {
        match (showing, self.frozen.get()) {
            (Some(_), None) => self.frozen.set(Some(self.real_now())),
            (None, Some(since)) => {
                self.paused
                    .set(self.paused.get() + (self.real_now() - since));
                self.frozen.set(None);
                let held = std::mem::take(&mut *self.held.borrow_mut());
                for frame in held {
                    let _: JsValue = self.request(frame);
                }
            }
            (Some(_), Some(_)) | (None, None) => {}
        }
    }
}

thread_local! {
    static STARTED: Cell<bool> = const { Cell::new(false) };
    static MENU: RefCell<Option<Rc<Menu>>> = const { RefCell::new(None) };
}

/// Démarre le menu de la console dans cette page, une seule fois.
pub(crate) fn start() {
    if STARTED.replace(true) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    settle(&document);
    no_zoom(&document);
    let menu = Rc::new(Menu {
        frames: Frames::install(&window),
        sound: Sound::install(&window),
        window,
        document,
        inner: RefCell::new(Inner::default()),
    });
    // En capture sur la fenêtre : le menu voit les touches avant le jeu.
    let capture = AddEventListenerOptions::new();
    capture.set_capture(true);
    for (name, down) in [("keydown", true), ("keyup", false)] {
        let listener = Rc::clone(&menu);
        let on_key = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if down {
                listener.key_down(&event);
            } else {
                listener.key_up(&event);
            }
        });
        let _: Result<(), JsValue> = menu
            .window
            .add_event_listener_with_callback_and_add_event_listener_options(
                name,
                on_key.as_ref().unchecked_ref(),
                &capture,
            );
        on_key.forget();
    }
    let on_blur = {
        let menu = Rc::clone(&menu);
        Closure::<dyn FnMut()>::new(move || menu.drop_hold())
    };
    let _: Result<(), JsValue> = menu
        .window
        .add_event_listener_with_callback("blur", on_blur.as_ref().unchecked_ref());
    on_blur.forget();
    menu.connect();
    MENU.with_borrow_mut(|kept| *kept = Some(menu));
}

/// Pose les réglages de la console dans la page ([`PAGE_STYLE`]). En tête du
/// document : une règle que le jeu écrit pour `html` passe après, et l'emporte.
/// Le `touch-action` d'un élément se combine avec celui de ses parents : une
/// manette qui dit `none` reste `none`.
fn settle(document: &Document) {
    let Ok(style) = document.create_element("style") else {
        return;
    };
    style.set_text_content(Some(PAGE_STYLE));
    let head = document.query_selector("head").ok().flatten();
    let Some(parent) = head.or_else(|| document.document_element()) else {
        return;
    };
    let _: Result<web_sys::Node, JsValue> =
        parent.insert_before(&style, parent.first_child().as_ref());
}

/// Un appui : où, et quand.
#[derive(Clone, Copy)]
struct Tap {
    at: f64,
    x: f64,
    y: f64,
}

impl Tap {
    fn near(self, x: f64, y: f64, reach: f64) -> bool {
        (self.x - x).abs() <= reach && (self.y - y).abs() <= reach
    }
}

/// Empêche le zoom là où `touch-action` ne suffit pas ([`PAGE_STYLE`]).
///
/// - Le double appui : la page refuse la fin du deuxième appui, ce qui annule
///   le zoom. Le navigateur ne donne alors plus le clic de cet appui : la
///   console le donne elle-même, pour qu'un bouton pressé deux fois vite
///   reçoive ses deux clics. Un champ de saisie garde ses gestes à lui.
/// - Le pincement (deux pouces sur une manette) : Safari l'annonce par
///   `gesturestart`, que la page refuse.
///
/// Les événements de pointeur (`pointerdown`, `pointerup`) ne changent pas.
fn no_zoom(document: &Document) {
    let capture = |passive: bool| {
        let options = AddEventListenerOptions::new();
        options.set_capture(true);
        options.set_passive(passive);
        options
    };
    let down: Rc<Cell<Option<Tap>>> = Rc::new(Cell::new(None));
    let on_start = {
        let down = Rc::clone(&down);
        Closure::<dyn FnMut(TouchEvent)>::new(move |event: TouchEvent| {
            // Un deuxième doigt : ce n'est plus un appui.
            let alone = event.touches().length() == 1;
            down.set(
                event
                    .changed_touches()
                    .get(0)
                    .filter(|_| alone)
                    .map(|touch| Tap {
                        at: event.time_stamp(),
                        x: f64::from(touch.client_x()),
                        y: f64::from(touch.client_y()),
                    }),
            );
        })
    };
    let _: Result<(), JsValue> = document
        .add_event_listener_with_callback_and_add_event_listener_options(
            "touchstart",
            on_start.as_ref().unchecked_ref(),
            &capture(true),
        );
    on_start.forget();
    let last: Cell<Option<Tap>> = Cell::new(None);
    let on_end = Closure::<dyn FnMut(TouchEvent)>::new(move |event: TouchEvent| {
        let tap = down
            .take()
            .zip(event.changed_touches().get(0))
            .and_then(|(down, touch)| {
                let (x, y) = (f64::from(touch.client_x()), f64::from(touch.client_y()));
                (event.touches().length() == 0 && down.near(x, y, TAP_REACH)).then_some(Tap {
                    at: event.time_stamp(),
                    x,
                    y,
                })
            });
        let before = last.replace(tap);
        let (Some(tap), Some(before)) = (tap, before) else {
            return;
        };
        if tap.at - before.at > DOUBLE_TAP_MS || !before.near(tap.x, tap.y, DOUBLE_TAP_REACH) {
            return;
        }
        let target = event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok());
        if target.as_ref().is_some_and(writes) {
            return;
        }
        event.prevent_default();
        if let Some(pressed) = target.and_then(|target| target.dyn_into::<HtmlElement>().ok()) {
            pressed.click();
        }
    });
    let _: Result<(), JsValue> = document
        .add_event_listener_with_callback_and_add_event_listener_options(
            "touchend",
            on_end.as_ref().unchecked_ref(),
            &capture(false),
        );
    on_end.forget();
    let on_pinch = Closure::<dyn FnMut(Event)>::new(|event: Event| event.prevent_default());
    for name in ["gesturestart", "gesturechange"] {
        let _: Result<(), JsValue> = document
            .add_event_listener_with_callback_and_add_event_listener_options(
                name,
                on_pinch.as_ref().unchecked_ref(),
                &capture(false),
            );
    }
    on_pinch.forget();
}

/// Un endroit où l'on écrit : ses gestes (placer le curseur, choisir un mot)
/// restent ceux du navigateur.
fn writes(element: &Element) -> bool {
    element
        .closest("input, textarea, select, [contenteditable]")
        .is_ok_and(|found| found.is_some())
}

/// Le jeu demande à la console d'arrêter le temps, ou de le rendre (voir
/// [`crate::web::pause`]).
pub(crate) fn ask_console(order: &str) {
    MENU.with_borrow(|menu| {
        if let Some(menu) = menu {
            menu.send(order);
        }
    });
}

impl Inner {
    fn showing(&self) -> Option<Showing> {
        if self.closed {
            return (!self.local).then_some(Showing::Closed);
        }
        if self.entry == Entry::Out && !self.local {
            return Some(Showing::Uninvited);
        }
        match (self.state.pause, self.state.tenu, self.local) {
            (true, _, true) => Some(Showing::Menu),
            (false, true, true) => Some(Showing::Held),
            (true, _, false) | (false, true, false) => Some(Showing::Paused),
            (false, false, _) => None,
        }
    }

    fn items(&self) -> Vec<Item> {
        if self.asking == Asking::Restart {
            return vec![Item::BackToMenu, Item::Restart];
        }
        let mut items = vec![Item::Resume];
        if self.state.recommencer {
            items.push(Item::AskRestart);
        }
        if self.state.etagere.is_some() {
            items.push(Item::Quit);
        }
        items
    }

    /// Ce que la page montre par-dessus le jeu, écrit en HTML.
    fn html(&self) -> String {
        let mut html = String::from(STYLE);
        match self.pill {
            Some(Pill::Holding) => {
                html.push_str("<div class=\"pill filling\">Menu de la console<i></i></div>");
            }
            Some(Pill::Hint) => html
                .push_str("<div class=\"pill\">Maintiens Échap pour le menu de la console</div>"),
            None => {}
        }
        let panel = match self.showing() {
            None | Some(Showing::Held) => return html,
            Some(Showing::Paused) => {
                "<div class=\"panel lone\"><h1>Pause</h1><p>L'hôte a mis le jeu en pause.</p></div>"
                    .to_owned()
            }
            Some(Showing::Closed) => "<div class=\"panel lone\"><h1>Ce jeu est fermé</h1>\
                 <p>Pour le prochain jeu, scanne son QR code sur la télé.</p></div>"
                .to_owned(),
            Some(Showing::Uninvited) => format!(
                "<div class=\"panel lone\"><h1>{}</h1>\
                 <p>Pour jouer, scanne le QR code de la télé.</p></div>",
                escape(self.state.jeu.as_deref().unwrap_or("Un jeu a commencé"))
            ),
            Some(Showing::Menu) => self.menu_html(),
        };
        html.push_str("<div class=\"veil\">");
        html.push_str(&panel);
        html.push_str("</div>");
        html
    }

    fn menu_html(&self) -> String {
        let confirming = self.asking == Asking::Restart;
        let label = if confirming { "Recommencer ?" } else { "Pause" };
        let title = escape(self.state.jeu.as_deref().unwrap_or("Le jeu"));
        let mut html =
            format!("<div class=\"panel\"><div class=\"label\">{label}</div><h1>{title}</h1>");
        if confirming {
            html.push_str("<p>La partie en cours sera perdue.</p>");
        }
        for (index, item) in self.items().into_iter().enumerate() {
            let on = if index == self.choice {
                " class=\"on\""
            } else {
                ""
            };
            let _: std::fmt::Result = write!(
                html,
                "<button data-i=\"{index}\"{on}>{}</button>",
                item.label()
            );
        }
        if self.state.qr {
            html.push_str(
                "<div class=\"qr\"><img src=\"/qr.svg\" alt=\"QR code des téléphones\">\
                 Pour rejoindre avec un téléphone</div>",
            );
        }
        html.push_str(&delays_html(&self.state.delais));
        html.push_str(
            "<div class=\"keys\">↑ ↓ pour choisir · Entrée pour valider · Échap pour reprendre</div></div>",
        );
        html
    }
}

/// Au-delà, un téléphone se sent en retard dans un jeu rapide : le menu le
/// fait ressortir.
const SLOW_MS: u32 = 150;

/// Le délai de chaque téléphone : le temps d'un aller-retour entre lui et le
/// jeu. C'est ce que l'hôte regarde quand un joueur dit que « ça rame ».
fn delays_html(delays: &[u32]) -> String {
    if delays.is_empty() {
        return String::new();
    }
    let each: Vec<String> = delays
        .iter()
        .map(|delay| {
            if *delay >= SLOW_MS {
                format!("<b>{delay} ms</b>")
            } else {
                format!("{delay} ms")
            }
        })
        .collect();
    format!(
        "<div class=\"delays\">Délai des téléphones : {}</div>",
        each.join(" · ")
    )
}

/// Un texte venu du jeu, sans rien qui se lise comme du HTML.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Menu {
    /// Là où le menu se dessine : à part du jeu, dont les styles n'entrent pas.
    fn root(self: &Rc<Self>) -> Option<ShadowRoot> {
        if let Some(root) = self.inner.borrow().root.clone() {
            return Some(root);
        }
        let host = self.document.create_element("div").ok()?;
        host.set_id("soiree-console");
        let root = host
            .attach_shadow(&ShadowRootInit::new(ShadowRootMode::Closed))
            .ok()?;
        let parent: Element = match self.document.body() {
            Some(body) => body.into(),
            None => self.document.document_element()?,
        };
        let _: web_sys::Node = parent.append_child(&host).ok()?;
        let on_click = {
            let menu = Rc::clone(self);
            Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let index = event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                    .and_then(|element| element.get_attribute("data-i"))
                    .and_then(|index| index.parse::<usize>().ok());
                if let Some(index) = index {
                    menu.run(index);
                }
            })
        };
        let _: Result<(), JsValue> =
            root.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
        on_click.forget();
        self.inner.borrow_mut().root = Some(root.clone());
        Some(root)
    }

    fn render(self: &Rc<Self>) {
        let (html, drawn, showing) = {
            let inner = self.inner.borrow();
            let showing = inner.showing();
            let something = showing.is_some() || inner.pill.is_some();
            (inner.html(), something || inner.root.is_some(), showing)
        };
        if let Some(frames) = &self.frames {
            frames.hold(showing);
        }
        self.sound.hold(&self.document, showing);
        if drawn && let Some(root) = self.root() {
            root.set_inner_html(&html);
        }
    }

    fn send(&self, order: &str) {
        if let Some(socket) = &self.inner.borrow().socket
            && socket.ready_state() == WebSocket::OPEN
        {
            let _: Result<(), JsValue> = socket.send_with_str(order);
        }
    }

    /// Fait ce que dit la ligne `index` du menu.
    fn run(self: &Rc<Self>, index: usize) {
        let item = self.inner.borrow().items().get(index).copied();
        match item {
            Some(Item::Resume) => self.send("reprendre"),
            Some(Item::Quit) => self.send("quitter"),
            Some(Item::Restart) => self.send("recommencer"),
            Some(Item::AskRestart) => self.ask(Asking::Restart),
            Some(Item::BackToMenu) => self.ask(Asking::What),
            None => {}
        }
    }

    fn ask(self: &Rc<Self>, asking: Asking) {
        {
            let mut inner = self.inner.borrow_mut();
            inner.asking = asking;
            inner.choice = 0;
        }
        self.render();
    }

    fn later(&self, millis: i32, run: impl FnOnce() + 'static) -> Option<i32> {
        let run = Closure::once_into_js(run);
        self.window
            .set_timeout_with_callback_and_timeout_and_arguments_0(run.unchecked_ref(), millis)
            .ok()
    }

    fn clear_hint(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(timer) = inner.hint_timer.take() {
            self.window.clear_timeout_with_handle(timer);
        }
        if inner.pill == Some(Pill::Hint) {
            inner.pill = None;
        }
    }

    /// Oublie l'appui en cours sur Échap, sans rien rendre au jeu.
    fn drop_hold(self: &Rc<Self>) {
        let dropped = {
            let mut inner = self.inner.borrow_mut();
            let dropped = inner.hold.take();
            if dropped.is_some() && inner.pill == Some(Pill::Holding) {
                inner.pill = None;
            }
            dropped
        };
        if let Some(hold) = dropped {
            if let Some(timer) = hold.timer {
                self.window.clear_timeout_with_handle(timer);
            }
            self.render();
        }
    }

    fn key_down(self: &Rc<Self>, event: &KeyboardEvent) {
        // Un appui que le menu rend lui-même au jeu : il passe.
        if !event.is_trusted() {
            return;
        }
        let key = event.key();
        let (showing, local, holding) = {
            let inner = self.inner.borrow();
            (inner.showing(), inner.local, inner.hold.is_some())
        };
        if let Some(showing) = showing.filter(|showing| showing.covers()) {
            event.stop_immediate_propagation();
            // Les raccourcis du navigateur (recharger, fermer) restent à l'hôte.
            if !event.meta_key() && !event.ctrl_key() {
                event.prevent_default();
            }
            if showing == Showing::Menu && !event.repeat() {
                self.menu_key(&key);
            }
            return;
        }
        if !local || key != ESCAPE {
            return;
        }
        // Retenu : court, il sera rendu au jeu ; long, il ouvre le menu.
        event.stop_immediate_propagation();
        if event.repeat() || holding {
            return;
        }
        self.clear_hint();
        let timer = {
            let menu = Rc::clone(self);
            self.later(HOLD_MS, move || menu.held_long_enough())
        };
        {
            let mut inner = self.inner.borrow_mut();
            inner.hold = Some(Hold {
                target: event.target(),
                timer,
            });
            inner.pill = Some(Pill::Holding);
        }
        self.render();
    }

    /// Échap est resté enfoncé : le menu s'ouvre.
    fn held_long_enough(self: &Rc<Self>) {
        {
            let mut inner = self.inner.borrow_mut();
            if let Some(hold) = &mut inner.hold {
                hold.timer = None;
            }
            inner.pill = None;
            inner.choice = 0;
            inner.asking = Asking::What;
        }
        self.render();
        self.send("pause");
    }

    fn key_up(self: &Rc<Self>, event: &KeyboardEvent) {
        if !event.is_trusted() {
            return;
        }
        let escape = event.key() == ESCAPE;
        let (covered, local) = {
            let inner = self.inner.borrow();
            (inner.showing().is_some_and(Showing::covers), inner.local)
        };
        if covered {
            event.stop_immediate_propagation();
            if escape {
                self.drop_hold();
            }
            return;
        }
        if !local || !escape {
            return;
        }
        event.stop_immediate_propagation();
        let released = self.inner.borrow_mut().hold.take();
        let Some(hold) = released else {
            return;
        };
        self.inner.borrow_mut().pill = None;
        // Relâché avant le délai : c'était pour le jeu.
        if let Some(timer) = hold.timer {
            self.window.clear_timeout_with_handle(timer);
            self.replay(hold.target);
            self.hint();
        }
        self.render();
    }

    /// Rend au jeu l'appui court qu'on avait retenu.
    fn replay(&self, target: Option<EventTarget>) {
        let connected = target.filter(|target| {
            target
                .dyn_ref::<web_sys::Node>()
                .is_none_or(web_sys::Node::is_connected)
        });
        let target: EventTarget = match connected {
            Some(target) => target,
            None => match self.document.active_element() {
                Some(element) => element.into(),
                None => self.window.clone().into(),
            },
        };
        let init = KeyboardEventInit::new();
        init.set_key(ESCAPE);
        init.set_code(ESCAPE);
        init.set_key_code(ESCAPE_CODE);
        init.set_which(ESCAPE_CODE);
        init.set_bubbles(true);
        init.set_cancelable(true);
        init.set_composed(true);
        for name in ["keydown", "keyup"] {
            if let Ok(event) = KeyboardEvent::new_with_keyboard_event_init_dict(name, &init) {
                let _: Result<bool, JsValue> = target.dispatch_event(&event);
            }
        }
    }

    /// La première fois qu'un appui court passe, dit le geste du menu.
    fn hint(self: &Rc<Self>) {
        if self.inner.borrow().hinted {
            return;
        }
        let timer = {
            let menu = Rc::clone(self);
            self.later(HINT_MS, move || {
                menu.clear_hint();
                menu.render();
            })
        };
        let mut inner = self.inner.borrow_mut();
        inner.hinted = true;
        inner.pill = Some(Pill::Hint);
        inner.hint_timer = timer;
    }

    fn menu_key(self: &Rc<Self>, key: &str) {
        let (count, choice, holding) = {
            let inner = self.inner.borrow();
            (
                inner.items().len().max(1),
                inner.choice,
                inner.hold.is_some(),
            )
        };
        let moved = match key {
            "ArrowDown" => choice.wrapping_add(1).checked_rem(count),
            "ArrowUp" => choice
                .wrapping_add(count)
                .wrapping_sub(1)
                .checked_rem(count),
            "Enter" | " " => return self.run(choice),
            // L'appui qui a ouvert le menu n'est pas encore relâché.
            ESCAPE if !holding => {
                self.inner.borrow_mut().asking = Asking::What;
                return self.send("reprendre");
            }
            _ => None,
        };
        if let Some(moved) = moved {
            self.inner.borrow_mut().choice = moved;
            self.render();
        }
    }

    fn on_state(self: &Rc<Self>, next: State) {
        if let Some(local) = next.ici {
            {
                let mut inner = self.inner.borrow_mut();
                inner.local = local;
                inner.entry = if next.billet == Some(false) {
                    Entry::Out
                } else {
                    Entry::In
                };
            }
            self.render();
            return;
        }
        let leave = {
            let mut inner = self.inner.borrow_mut();
            // Un autre jeu répond à cette adresse : on repart de sa page.
            if let (Some(seen), Some(now)) = (&inner.party, &next.partie)
                && seen != now
            {
                drop(inner);
                let _: Result<(), JsValue> = self.window.location().reload();
                return;
            }
            if inner.party.is_none() {
                inner.party.clone_from(&next.partie);
            }
            if next.etagere.is_some() {
                inner.shelf.clone_from(&next.etagere);
            }
            if next.pause && !inner.state.pause {
                inner.choice = 0;
                inner.asking = Asking::What;
            }
            let leave = next.ferme;
            inner.state = next;
            inner.closed = false;
            leave
        };
        if leave {
            self.leave();
        } else {
            self.render();
        }
    }

    /// Le jeu est fermé : l'écran retourne à l'étagère, un téléphone attend.
    fn leave(self: &Rc<Self>) {
        let back = {
            let mut inner = self.inner.borrow_mut();
            inner.closed = true;
            inner.state.pause = false;
            inner.shelf.clone().filter(|_| inner.local)
        };
        match back {
            Some(address) => {
                let _: Result<(), JsValue> = self.window.location().set_href(&address);
            }
            None => self.render(),
        }
    }

    fn retry(self: &Rc<Self>) {
        let menu = Rc::clone(self);
        let _: Option<i32> = self.later(RETRY_MS, move || menu.connect());
    }

    fn connect(self: &Rc<Self>) {
        let location = self.window.location();
        let scheme = if location
            .protocol()
            .is_ok_and(|protocol| protocol == "https:")
        {
            "wss"
        } else {
            "ws"
        };
        let socket = location.host().ok().and_then(|host| {
            // Le billet de la page, s'il y en a un : `?c=…`.
            let shown = crate::web::shown().replacen('&', "?", 1);
            WebSocket::new(&format!("{scheme}://{host}/soiree/console{shown}")).ok()
        });
        let Some(socket) = socket else {
            return self.retry();
        };
        let on_open = {
            let menu = Rc::clone(self);
            Closure::<dyn FnMut()>::new(move || menu.inner.borrow_mut().lost = 0)
        };
        let on_message = {
            let menu = Rc::clone(self);
            Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                let state = event
                    .data()
                    .as_string()
                    .and_then(|text| serde_json::from_str::<State>(&text).ok());
                if let Some(state) = state {
                    menu.on_state(state);
                }
            })
        };
        let on_close = {
            let menu = Rc::clone(self);
            Closure::<dyn FnMut(CloseEvent)>::new(move |_: CloseEvent| {
                let gone = {
                    let mut inner = menu.inner.borrow_mut();
                    inner.lost = inner.lost.saturating_add(1);
                    // Le jeu s'est arrêté sans prévenir : l'étagère saura dire pourquoi.
                    inner.lost >= LOST_TRIES
                        && inner.party.is_some()
                        && inner.shelf.is_some()
                        && !inner.closed
                };
                if gone {
                    menu.leave();
                }
                menu.retry();
            })
        };
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        // Un lien par tentative : ses écouteurs vivent aussi longtemps que la page.
        on_open.forget();
        on_message.forget();
        on_close.forget();
        self.inner.borrow_mut().socket = Some(socket);
    }
}
