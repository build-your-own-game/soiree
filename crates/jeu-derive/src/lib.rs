//! Les macros de `jeu` (INTERFACE.md, § 2.2 et § 8.2) : `#[jeu::message]`,
//! `#[jeu::etat]` et `#[derive(Variantes)]`. Aucune logique de jeu.
//!
//! Elles écrivent des chemins absolus vers `::jeu` : `jeu` les réexporte, et se
//! nomme lui-même `jeu` (`extern crate self as jeu`) pour s'en servir sur ses
//! propres types.
//!
//! | Macro | Ce qu'elle écrit | Ce qu'elle refuse |
//! |---|---|---|
//! | `#[jeu::message]` | `Clone`, `Debug`, `PartialEq`, `Serialize`, `Deserialize`, `Forme`, `Etat`, `Message` | un champ qui n'est pas `Message` ; les attributs `serde` tolérants (REM-60, PUN-91, SAL-119) ; un type à mutabilité intérieure |
//! | `#[jeu::etat]` | `Clone`, `Debug`, `Forme`, `Etat` | un champ qui n'est pas `Etat` ; un type à mutabilité intérieure |
//! | `#[derive(Variantes)]` | `Variantes::TOUTES` | un variant à champs |
//!
//! **L'empreinte de forme** (`Forme::EMPREINTE`) est un FNV-1a 64, calculé à la
//! compilation, de la sorte (message ou état), du nom du type, du nom de chaque
//! variant et de chaque champ, dans l'ordre, et de l'empreinte de la forme de chaque
//! champ. Renommer, ajouter, retirer ou déplacer un champ ou un variant la change.
//! Elle ne dépend ni du chemin du module ni de la machine : deux compilations du même
//! type la donnent égale.

#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Flux, TokenTree};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Index, Type, parse_macro_input};

/// Ce qui traverse une frontière : vue, geste, avis, garde, résumé.
///
/// Écrit `Clone`, `Debug`, `PartialEq`, `Serialize`, `Deserialize`, `Forme`, `Etat`
/// et `Message`. Chaque champ doit être `Message` : un `Secret`, un `Partage` ou un
/// flottant nu n'en sont pas, et ne compilent pas.
#[proc_macro_attribute]
pub fn message(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(e) = sans_argument(attr, "message") {
        return e;
    }
    let entree = parse_macro_input!(item as DeriveInput);
    match ecrire(&entree, Sorte::Message) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Ce qui peut vivre dans l'état : `Clone`, `Debug`, `Forme`, `Etat` (projection,
/// forme canonique). Chaque champ doit être `Etat` ; aucun type à mutabilité
/// intérieure n'en est un.
#[proc_macro_attribute]
pub fn etat(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Some(e) = sans_argument(attr, "etat") {
        return e;
    }
    let entree = parse_macro_input!(item as DeriveInput);
    match ecrire(&entree, Sorte::Etat) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// `const TOUTES: &'static [Self]` : chaque variant d'une enum sans champ, dans
/// l'ordre de déclaration.
#[proc_macro_derive(Variantes)]
pub fn variantes(item: TokenStream) -> TokenStream {
    let entree = parse_macro_input!(item as DeriveInput);
    match ecrire_variantes(&entree) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn sans_argument(attr: TokenStream, nom: &str) -> Option<TokenStream> {
    if attr.is_empty() {
        return None;
    }
    let message = format!("#[jeu::{nom}] ne prend aucun argument");
    Some(
        syn::Error::new(
            Flux::from(attr)
                .into_iter()
                .next()
                .map_or_else(Span::call_site, |t| t.span()),
            message,
        )
        .to_compile_error()
        .into(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sorte {
    Message,
    Etat,
}

impl Sorte {
    const fn nom(self) -> &'static str {
        match self {
            Sorte::Message => "message",
            Sorte::Etat => "etat",
        }
    }
}

/// Les attributs `serde` qui rendent un décodage tolérant ou ambigu : une étiquette
/// interne ou absente que postcard ne lit pas (PUN-91), une valeur par défaut, un
/// champ sauté, un décodeur écrit à la main, un variant fourre-tout (REM-60, SAL-119).
const SERDE_REFUSES: &[&str] = &[
    "untagged",
    "tag",
    "content",
    "flatten",
    "default",
    "skip",
    "skip_serializing",
    "skip_deserializing",
    "skip_serializing_if",
    "with",
    "serialize_with",
    "deserialize_with",
    "other",
];

/// Les types à mutabilité intérieure, ou d'ordre d'itération variable : un refus ou
/// une lecture pourrait écrire à travers eux (§ 2.7), ou deux exécutions diverger.
const TYPES_REFUSES: &[(&str, &str)] = &[
    (
        "Cell",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "RefCell",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "UnsafeCell",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "SyncUnsafeCell",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "OnceCell",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "LazyCell",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "OnceLock",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "LazyLock",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "Mutex",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "RwLock",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "Rc",
        "un `Rc` se partage et se modifie hors de l'état : `jeu::Partage` (TL-20)",
    ),
    (
        "Arc",
        "un `Arc` se partage et se modifie hors de l'état : `jeu::Partage` (TL-20)",
    ),
    ("Weak", "un pointeur faible ne se copie pas avec l'état"),
    (
        "HashMap",
        "ordre d'itération différent entre natif et WebAssembly : `BTreeMap`",
    ),
    (
        "HashSet",
        "ordre d'itération différent entre natif et WebAssembly : `BTreeSet`",
    ),
    (
        "AtomicBool",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicU8",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicU16",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicU32",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicU64",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicUsize",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicI8",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicI16",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicI32",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicI64",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicIsize",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
    (
        "AtomicPtr",
        "mutabilité intérieure : un refus ou une lecture pourrait écrire l'état",
    ),
];

/// Un champ, dans l'ordre de déclaration.
struct Champ<'a> {
    /// Son nom, ou son rang pour un champ sans nom.
    nom: String,
    /// Comment l'atteindre : `a`, ou `0`.
    acces: Flux,
    /// La variable qui le lie dans un motif.
    lien: Ident,
    ty: &'a Type,
}

fn champs(f: &Fields) -> Vec<Champ<'_>> {
    f.iter()
        .enumerate()
        .map(|(i, c)| {
            if let Some(id) = &c.ident {
                Champ {
                    nom: id.to_string(),
                    acces: quote!(#id),
                    lien: format_ident!("champ_{}", id),
                    ty: &c.ty,
                }
            } else {
                let index = Index::from(i);
                Champ {
                    nom: i.to_string(),
                    acces: quote!(#index),
                    lien: format_ident!("champ_{}", i),
                    ty: &c.ty,
                }
            }
        })
        .collect()
}

/// Le motif qui lie chaque champ : `{ a: champ_a, .. }`, `(champ_0, ..)` ou rien.
fn motif(f: &Fields, cs: &[Champ<'_>]) -> Flux {
    let liens = cs.iter().map(|c| &c.lien);
    match f {
        Fields::Named(_) => {
            let noms = cs.iter().map(|c| &c.acces);
            quote!({ #( #noms: #liens ),* })
        }
        Fields::Unnamed(_) => quote!(( #( #liens ),* )),
        Fields::Unit => quote!(),
    }
}

/// Le constructeur depuis des expressions par champ.
fn construire(f: &Fields, cs: &[Champ<'_>], valeurs: &[Flux]) -> Flux {
    match f {
        Fields::Named(_) => {
            let noms = cs.iter().map(|c| &c.acces);
            quote!({ #( #noms: #valeurs ),* })
        }
        Fields::Unnamed(_) => quote!(( #( #valeurs ),* )),
        Fields::Unit => quote!(),
    }
}

fn verifier_serde(entree: &DeriveInput) -> syn::Result<()> {
    let mut attrs: Vec<&syn::Attribute> = entree.attrs.iter().collect();
    match &entree.data {
        Data::Struct(s) => attrs.extend(s.fields.iter().flat_map(|c| c.attrs.iter())),
        Data::Enum(e) => {
            for v in &e.variants {
                attrs.extend(v.attrs.iter());
                attrs.extend(v.fields.iter().flat_map(|c| c.attrs.iter()));
            }
        }
        Data::Union(_) => {}
    }
    for a in attrs {
        if !a.path().is_ident("serde") {
            continue;
        }
        let syn::Meta::List(liste) = &a.meta else {
            continue;
        };
        for t in liste.tokens.clone() {
            if let TokenTree::Ident(id) = t {
                let mot = id.to_string();
                if SERDE_REFUSES.contains(&mot.as_str()) {
                    return Err(syn::Error::new(
                        id.span(),
                        format!(
                            "#[serde({mot})] est refusé : un décodage doit être strict et sans \
                             ambiguïté (REM-60, PUN-91, SAL-119)"
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Cherche un type refusé dans les jetons d'un type de champ, à toute profondeur.
fn type_refuse(t: Flux) -> Option<(Span, &'static str, &'static str)> {
    for jeton in t {
        match jeton {
            TokenTree::Ident(id) => {
                let mot = id.to_string();
                if let Some((nom, raison)) = TYPES_REFUSES.iter().find(|(n, _)| *n == mot) {
                    return Some((id.span(), nom, raison));
                }
            }
            TokenTree::Group(g) => {
                if let Some(r) = type_refuse(g.stream()) {
                    return Some(r);
                }
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
    None
}

fn verifier_types(tous: &[&Type], sorte: Sorte) -> syn::Result<()> {
    for ty in tous {
        if let Some((span, nom, raison)) = type_refuse(quote!(#ty)) {
            return Err(syn::Error::new(
                span,
                format!(
                    "`{nom}` ne peut pas vivre dans un #[jeu::{}] : {raison}",
                    sorte.nom()
                ),
            ));
        }
    }
    Ok(())
}

/// L'empreinte de forme : la sorte, le nom, puis chaque variant et chaque champ.
fn empreinte(nom: &Ident, sorte: Sorte, data: &Data) -> Flux {
    let soi = [nom.to_string()];
    let racine = quote!(::jeu::cache::forme);
    let sorte_nom = match (sorte, data) {
        (Sorte::Message, Data::Enum(_)) => "message enum",
        (Sorte::Message, Data::Struct(_) | Data::Union(_)) => "message struct",
        (Sorte::Etat, Data::Enum(_)) => "etat enum",
        (Sorte::Etat, Data::Struct(_) | Data::Union(_)) => "etat struct",
    };
    let nom_texte = nom.to_string();
    let mut pas = Vec::new();
    pas.push(quote!(let h = #racine::depart(#sorte_nom);));
    pas.push(quote!(let h = #racine::mot(h, #nom_texte);));
    let ajouter_champs = |f: &Fields, pas: &mut Vec<Flux>| {
        let cs = champs(f);
        let n = cs.len() as u64;
        pas.push(quote!(let h = #racine::entier(h, #n);));
        for c in cs {
            let n = &c.nom;
            let ty = c.ty;
            pas.push(quote!(let h = #racine::mot(h, #n);));
            // Un champ qui nomme le type lui-même : sa forme est celle du type, en cours
            // de calcul ; on en note la récursion, pas l'empreinte.
            if mentionne(quote!(#ty), &soi) {
                pas.push(quote!(let h = #racine::mot(h, "recursion");));
            } else {
                pas.push(quote!(let h = #racine::entier(h, <#ty as ::jeu::Forme>::EMPREINTE);));
            }
        }
    };
    match data {
        Data::Struct(s) => ajouter_champs(&s.fields, &mut pas),
        Data::Enum(e) => {
            let n = e.variants.len() as u64;
            pas.push(quote!(let h = #racine::entier(h, #n);));
            for v in &e.variants {
                let vn = v.ident.to_string();
                pas.push(quote!(let h = #racine::mot(h, #vn);));
                ajouter_champs(&v.fields, &mut pas);
            }
        }
        Data::Union(_) => {}
    }
    quote!({ #( #pas )* h })
}

fn tous_les_types(data: &Data) -> Vec<&Type> {
    match data {
        Data::Struct(s) => s.fields.iter().map(|c| &c.ty).collect(),
        Data::Enum(e) => e
            .variants
            .iter()
            .flat_map(|v| v.fields.iter().map(|c| &c.ty))
            .collect(),
        Data::Union(_) => Vec::new(),
    }
}

fn ecrire(entree: &DeriveInput, sorte: Sorte) -> syn::Result<Flux> {
    if let Data::Union(u) = &entree.data {
        return Err(syn::Error::new(
            u.union_token.span,
            format!(
                "#[jeu::{}] ne s'applique qu'à une struct ou une enum",
                sorte.nom()
            ),
        ));
    }
    if sorte == Sorte::Message {
        verifier_serde(entree)?;
    }
    let types = tous_les_types(&entree.data);
    verifier_types(&types, sorte)?;

    let nom = &entree.ident;
    let (imp, ty_g, ou) = entree.generics.split_for_impl();
    let predicats_existants = ou.map(|w| {
        let p = w.predicates.iter();
        quote!(#( #p, )*)
    });
    // Seuls les champs qui nomment un paramètre de type reçoivent une borne, comme le
    // fait `serde` : borner un champ qui nomme le type lui-même (une enum récursive)
    // ferait boucler le compilateur. Les autres sont vérifiés par le corps des impls.
    let parametres: Vec<String> = entree
        .generics
        .type_params()
        .map(|p| p.ident.to_string())
        .collect();
    let bornes: Vec<&Type> = types
        .iter()
        .copied()
        .filter(|t| {
            mentionne(quote!(#t), &parametres) && !mentionne(quote!(#t), &[nom.to_string()])
        })
        .collect();
    let borne = |t: Flux| {
        let types = bornes.iter();
        quote!(where #predicats_existants #( #types: #t, )*)
    };
    let ou_forme = borne(quote!(::jeu::Forme));
    let ou_etat = borne(quote!(::jeu::Etat));
    // Chaque champ d'un message est un message : un `Secret` n'y compile pas (§ 2.2).
    // Ici, tous les champs sont bornés, sauf ceux qui nomment le type lui-même.
    let non_recursifs = types
        .iter()
        .filter(|t| !mentionne(quote!(#t), &[nom.to_string()]));
    let ou_message = quote!(where #predicats_existants #( #non_recursifs: ::jeu::Message, )*);
    let emp = empreinte(nom, sorte, &entree.data);

    let derive = derive_de(sorte);
    let secret = secret_de(sorte, &types, nom);

    let corps_etat = match sorte {
        Sorte::Message => quote! {
            fn projeter(&self, _: &::jeu::Regard) -> Self {
                ::core::clone::Clone::clone(self)
            }
            fn ecrire(&self, e: &mut ::jeu::cache::Ecrivain<'_>) {
                e.message(self);
            }
            fn lire(l: &mut ::jeu::cache::Liseur<'_>) -> ::core::result::Result<Self, ::jeu::cache::Illisible> {
                l.message()
            }
        },
        Sorte::Etat => corps_etat(nom, &entree.data),
    };

    let message = match sorte {
        Sorte::Message => quote! {
            #[automatically_derived]
            impl #imp ::jeu::Message for #nom #ty_g #ou_message {}
        },
        Sorte::Etat => quote!(),
    };

    Ok(quote! {
        #derive
        #entree

        #[automatically_derived]
        impl #imp ::jeu::Forme for #nom #ty_g #ou_forme {
            const EMPREINTE: u64 = #emp;
            const SECRET: ::jeu::Contient = #secret;
            const VIDE: ::jeu::Contient = ::jeu::Contient::Non;
        }

        #[automatically_derived]
        impl #imp ::jeu::Etat for #nom #ty_g #ou_etat {
            #corps_etat
        }

        #message
    })
}

/// Un des noms apparaît-il dans ces jetons, à toute profondeur ?
fn mentionne(t: Flux, noms: &[String]) -> bool {
    t.into_iter().any(|j| match j {
        TokenTree::Ident(id) => noms.iter().any(|n| id == n),
        TokenTree::Group(g) => mentionne(g.stream(), noms),
        TokenTree::Punct(_) | TokenTree::Literal(_) => false,
    })
}

/// Projection et forme canonique champ par champ, pour `#[jeu::etat]`.
fn corps_etat(nom: &Ident, data: &Data) -> Flux {
    match data {
        Data::Struct(s) => {
            let cs = champs(&s.fields);
            let m = motif(&s.fields, &cs);
            let liens: Vec<&Ident> = cs.iter().map(|c| &c.lien).collect();
            let projetes: Vec<Flux> = liens
                .iter()
                .map(|l| quote!(::jeu::Etat::projeter(#l, r)))
                .collect();
            let lus: Vec<Flux> = cs
                .iter()
                .map(|c| {
                    let ty = c.ty;
                    quote!(<#ty as ::jeu::Etat>::lire(l)?)
                })
                .collect();
            let p = construire(&s.fields, &cs, &projetes);
            let lu = construire(&s.fields, &cs, &lus);
            quote! {
                fn projeter(&self, r: &::jeu::Regard) -> Self {
                    let #nom #m = self;
                    #nom #p
                }
                fn ecrire(&self, e: &mut ::jeu::cache::Ecrivain<'_>) {
                    let #nom #m = self;
                    #( ::jeu::Etat::ecrire(#liens, e); )*
                }
                fn lire(l: &mut ::jeu::cache::Liseur<'_>) -> ::core::result::Result<Self, ::jeu::cache::Illisible> {
                    ::core::result::Result::Ok(#nom #lu)
                }
            }
        }
        Data::Enum(e) => {
            let mut bras_projeter = Vec::new();
            let mut bras_ecrire = Vec::new();
            let mut bras_lire = Vec::new();
            for (i, v) in e.variants.iter().enumerate() {
                let vn = &v.ident;
                let cs = champs(&v.fields);
                let m = motif(&v.fields, &cs);
                let liens: Vec<&Ident> = cs.iter().map(|c| &c.lien).collect();
                let projetes: Vec<Flux> = liens
                    .iter()
                    .map(|l| quote!(::jeu::Etat::projeter(#l, r)))
                    .collect();
                let lus: Vec<Flux> = cs
                    .iter()
                    .map(|c| {
                        let ty = c.ty;
                        quote!(<#ty as ::jeu::Etat>::lire(l)?)
                    })
                    .collect();
                let p = construire(&v.fields, &cs, &projetes);
                let lu = construire(&v.fields, &cs, &lus);
                let rang = u32::try_from(i).unwrap_or(u32::MAX);
                bras_projeter.push(quote!(#nom::#vn #m => #nom::#vn #p,));
                bras_ecrire.push(quote!(#nom::#vn #m => {
                    e.variant(#rang);
                    #( ::jeu::Etat::ecrire(#liens, e); )*
                }));
                bras_lire.push(quote!(#rang => ::core::result::Result::Ok(#nom::#vn #lu),));
            }
            let projeter_corps = if e.variants.is_empty() {
                quote!(match *self {})
            } else {
                quote!(match self { #( #bras_projeter )* })
            };
            let ecrire_corps = if e.variants.is_empty() {
                quote!(match *self {})
            } else {
                quote!(match self { #( #bras_ecrire )* })
            };
            quote! {
                fn projeter(&self, r: &::jeu::Regard) -> Self {
                    #projeter_corps
                }
                fn ecrire(&self, e: &mut ::jeu::cache::Ecrivain<'_>) {
                    #ecrire_corps
                }
                fn lire(l: &mut ::jeu::cache::Liseur<'_>) -> ::core::result::Result<Self, ::jeu::cache::Illisible> {
                    match l.variant()? {
                        #( #bras_lire )*
                        _ => ::core::result::Result::Err(l.variant_inconnu()),
                    }
                }
            }
        }
        Data::Union(_) => quote!(),
    }
}

fn ecrire_variantes(entree: &DeriveInput) -> syn::Result<Flux> {
    let Data::Enum(e) = &entree.data else {
        return Err(syn::Error::new(
            entree.ident.span(),
            "#[derive(Variantes)] ne s'applique qu'à une enum sans champ",
        ));
    };
    for v in &e.variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new(
                v.ident.span(),
                format!(
                    "#[derive(Variantes)] : le variant `{}` a des champs ; seules les enums sans \
                     champ ont une liste finie de variantes",
                    v.ident
                ),
            ));
        }
    }
    let nom = &entree.ident;
    let (imp, ty_g, ou) = entree.generics.split_for_impl();
    let vs = e.variants.iter().map(|v| &v.ident);
    Ok(quote! {
        #[automatically_derived]
        impl #imp ::jeu::Variantes for #nom #ty_g #ou {
            const TOUTES: &'static [Self] = &[ #( #nom::#vs ),* ];
        }
    })
}

/// Les dérivations standard : `serde` et `PartialEq` pour un message, `Clone` et `Debug`
/// pour tous.
fn derive_de(sorte: Sorte) -> Flux {
    match sorte {
        Sorte::Message => quote! {
            #[derive(
                ::core::clone::Clone,
                ::core::fmt::Debug,
                ::core::cmp::PartialEq,
                ::jeu::cache::serde::Serialize,
                ::jeu::cache::serde::Deserialize
            )]
            #[serde(crate = "::jeu::cache::serde", bound = "")]
        },
        Sorte::Etat => quote! {
            #[derive(::core::clone::Clone, ::core::fmt::Debug)]
        },
    }
}

/// `Forme::SECRET` : un message n'en contient aucun ; un état, si l'un de ses champs en
/// contient un.
fn secret_de(sorte: Sorte, types: &[&Type], nom: &Ident) -> Flux {
    match sorte {
        // Un message ne contient aucun secret : chacun de ses champs est un message.
        Sorte::Message => quote!(::jeu::Contient::Non),
        Sorte::Etat => {
            // Un champ qui nomme le type lui-même ne contient rien de plus que lui.
            let ts = types
                .iter()
                .filter(|t| !mentionne(quote!(#t), &[nom.to_string()]));
            quote!({
                let c = ::jeu::Contient::Non;
                #( let c = ::jeu::cache::forme::ou(c, <#ts as ::jeu::Forme>::SECRET); )*
                c
            })
        }
    }
}
