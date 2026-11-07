//! Le chiffrement (PHASE-3, § 2.4 et § 2.6, P12) : la poignée de main entre un appareil
//! et l'autorité, les trames de transport, et les cases de la garde.
//!
//! - **La poignée de main** est `Noise_NKpsk0_25519_ChaChaPoly_BLAKE2s` ([`MOTIF`]), par
//!   `snow`. L'appareil ([`Initiateur`]) connaît la clé publique de l'autorité et le
//!   secret d'entrée, tous deux lus dans le fragment du QR ([`Fragment`]) ; l'autorité
//!   ([`repondre`]) a sa paire de clés ([`PaireAutorite`]) et le même secret. La psk de
//!   Noise fait 32 octets : elle se dérive du secret d'entrée de 16 octets
//!   ([`SecretDEntree::psk`]). **La poignée de main ne porte rien** : chacun de ses deux
//!   messages fait exactement [`TAILLE_POIGNEE`] octets, une charge vide, et tout autre
//!   est refusé ; le premier message de transport de l'appareil est son bonjour
//!   (SAL-17).
//! - **Les trames de transport** ([`Session::sceller`], [`Session::ouvrir`]) :
//!   ChaCha20-Poly1305 sous les clés de la poignée de main (le `Split` de Noise), le
//!   nonce de Noise (quatre octets nuls, puis le compteur en petit-boutiste), **un nonce
//!   explicite** en tête de la trame, la classe de la trame en données associées (le
//!   relais ne peut pas la changer), et une fenêtre de rejeu de [`FENETRE_DE_REJEU`]
//!   (une trame rejouée par le relais est refusée). Une trame va jusqu'à
//!   [`liaison::TRAME_MAX`], au-delà des 65 535 octets d'un message de transport de
//!   Noise : c'est pourquoi le transport n'est pas celui de `snow`.
//! - **Les cases de la garde** ([`sceller_case`], [`ouvrir_case`]) : XChaCha20-Poly1305,
//!   d'une clé dérivée de la clé de garde G ; les données associées portent la salle, la
//!   sorte et le numéro de la case, et l'empreinte de la case précédente : le relais ne
//!   peut ni lire, ni modifier, ni intervertir, ni rejouer une case.
//!
//! **Le hasard entre en paramètre** ([`Hasard`]), comme le temps : un générateur donné
//! par la coque. `snow` reçoit un résolveur qui tire de ce générateur ce qu'il faut à une
//! clé, et jamais le sien. Les tests tournent à graine fixe (SAL-09).
//!
//! Les dérivations (la psk, la preuve d'autorité, la clé des cases) sont des
//! `blake3::derive_key` à contexte nommé et versionné ; leurs octets sont figés par
//! `tests/chiffre.rs`.

use core::fmt;

use chacha20::XChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, Tag, XChaCha20Poly1305, XNonce};
use liaison::{Case, Classe, CodeSalle, TRAME_MAX};
use poly1305::Poly1305;
use poly1305::universal_hash::UniversalHash;
use snow::params::{CipherChoice, DHChoice, HashChoice, NoiseParams};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::types::{Cipher, Dh, Hash, Random};
use snow::{Builder, HandshakeState};

/// Le motif de Noise, tel que la spécification le nomme.
pub const MOTIF: &str = "Noise_NKpsk0_25519_ChaChaPoly_BLAKE2s";

/// Le prologue de la poignée de main : il nomme ce protocole, et sa version.
pub const PROLOGUE: &[u8] = b"soiree fil 1";

/// La taille de chacun des deux messages de la poignée de main : une clé éphémère de
/// 32 octets et l'étiquette de 16 octets d'une charge vide.
pub const TAILLE_POIGNEE: usize = 48;

/// Le nonce explicite en tête d'une trame.
const OCTETS_DU_NONCE: usize = 8;

/// L'étiquette d'authentification de ChaCha20-Poly1305.
const OCTETS_DE_L_ETIQUETTE: usize = 16;

/// Ce qu'une trame ajoute à son clair : son nonce et son étiquette.
pub const SURCOUT_TRAME: usize = OCTETS_DU_NONCE + OCTETS_DE_L_ETIQUETTE;

/// Le plus long clair qu'une trame porte : [`TRAME_MAX`] moins [`SURCOUT_TRAME`].
pub const CLAIR_MAX: usize = TRAME_MAX as usize - SURCOUT_TRAME;

/// La fenêtre de rejeu, en trames (choisi, PHASE-3, § 2.6) : une trame plus vieille que
/// les 1 024 dernières reçues est refusée, comme une trame déjà reçue. Sur une
/// WebSocket, rien ne se réordonne ; la fenêtre sert un canal non ordonné à venir (P11).
pub const FENETRE_DE_REJEU: u64 = 1024;

/// Le nonce d'une case de la garde : `XChaCha20`, 24 octets tirés au hasard.
const OCTETS_DU_NONCE_DE_CASE: usize = 24;

/// L'empreinte qui précède la première case d'une chaîne de garde.
pub const AVANT_LA_PREMIERE_CASE: [u8; 32] = [0; 32];

// ---------------------------------------------------------------------------
// Le hasard
// ---------------------------------------------------------------------------

/// Le hasard, donné par la coque (PHASE-3, § 2.2) : `crypto.getRandomValues` dans le
/// navigateur (SAL-93 ; la coque du Worker l'appelle directement, lot 10a), un générateur
/// à graine fixe dans les tests (SAL-09). `salle` ne tire jamais le sien.
pub trait Hasard {
    /// Remplit `octets` de hasard.
    fn remplir(&mut self, octets: &mut [u8]);
}

fn tirer<const N: usize>(h: &mut (impl Hasard + ?Sized)) -> [u8; N] {
    let mut o = [0u8; N];
    h.remplir(&mut o);
    o
}

// ---------------------------------------------------------------------------
// Les clés
// ---------------------------------------------------------------------------

/// Le secret d'entrée : 16 octets, dans le fragment du QR, tirés à chaque soirée. Il
/// réserve la salle à ceux qui ont vu le QR (P12).
#[derive(Clone, PartialEq, Eq)]
pub struct SecretDEntree([u8; 16]);

impl SecretDEntree {
    /// Un secret neuf.
    pub fn tirer(h: &mut (impl Hasard + ?Sized)) -> SecretDEntree {
        SecretDEntree(tirer(h))
    }

    /// Un secret gardé, ou lu dans le fragment.
    pub const fn depuis_octets(o: [u8; 16]) -> SecretDEntree {
        SecretDEntree(o)
    }

    /// Ses octets, pour le garder ou l'écrire dans le fragment.
    pub const fn octets(&self) -> &[u8; 16] {
        &self.0
    }

    /// La psk de Noise, 32 octets (la spécification de Noise, « Pre-shared symmetric
    /// keys », et `snow` n'en admettent pas d'autre longueur) :
    /// `blake3::derive_key("soiree secret d'entree 1", secret)`.
    pub fn psk(&self) -> [u8; 32] {
        blake3::derive_key("soiree secret d'entree 1", &self.0)
    }
}

impl fmt::Debug for SecretDEntree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretDEntree(…)")
    }
}

/// Une clé publique X25519 : celle de l'autorité, dans le fragment du QR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClePublique(pub [u8; 32]);

/// La paire de clés statique de l'autorité, tirée à chaque soirée, gardée avec G dans
/// le stockage de l'écran (P37).
#[derive(Clone)]
pub struct PaireAutorite {
    privee: [u8; 32],
    publique: ClePublique,
}

impl PaireAutorite {
    /// Une paire neuve.
    pub fn tirer(h: &mut (impl Hasard + ?Sized)) -> Result<PaireAutorite, ErreurChiffre> {
        PaireAutorite::depuis_privee(tirer(h))
    }

    /// La paire d'une clé privée gardée.
    pub fn depuis_privee(privee: [u8; 32]) -> Result<PaireAutorite, ErreurChiffre> {
        let paire = Builder::with_resolver(parametres()?, Box::new(Resolveur::new(privee)))
            .generate_keypair()
            .map_err(|_| ErreurChiffre::Interne)?;
        let publique: [u8; 32] = paire
            .public
            .as_slice()
            .try_into()
            .map_err(|_| ErreurChiffre::Interne)?;
        Ok(PaireAutorite {
            privee,
            publique: ClePublique(publique),
        })
    }

    /// La clé privée, pour la garder.
    pub const fn privee(&self) -> &[u8; 32] {
        &self.privee
    }

    /// La clé publique, pour le fragment.
    pub const fn publique(&self) -> ClePublique {
        self.publique
    }
}

impl fmt::Debug for PaireAutorite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PaireAutorite")
            .field("publique", &self.publique)
            .finish_non_exhaustive()
    }
}

/// La clé de garde G : elle lit tous les points, donc tous les secrets. **Jamais dans le
/// QR, ni dans aucune adresse** (P37).
#[derive(Clone, PartialEq, Eq)]
pub struct CleDeGarde([u8; 32]);

impl CleDeGarde {
    /// Une clé neuve.
    pub fn tirer(h: &mut (impl Hasard + ?Sized)) -> CleDeGarde {
        CleDeGarde(tirer(h))
    }

    /// Une clé gardée.
    pub const fn depuis_octets(o: [u8; 32]) -> CleDeGarde {
        CleDeGarde(o)
    }

    /// Ses octets, pour la garder.
    pub const fn octets(&self) -> &[u8; 32] {
        &self.0
    }

    /// La preuve d'autorité que le relais enregistre pour la salle (PHASE-3, § 2.7) :
    /// `blake3::derive_key("soiree preuve d'autorite 1", G)`.
    pub fn preuve(&self) -> [u8; 32] {
        blake3::derive_key("soiree preuve d'autorite 1", &self.0)
    }

    /// La clé des cases de la garde : `blake3::derive_key("soiree cases de garde 1", G)`.
    fn cle_des_cases(&self) -> [u8; 32] {
        blake3::derive_key("soiree cases de garde 1", &self.0)
    }
}

impl fmt::Debug for CleDeGarde {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CleDeGarde(…)")
    }
}

// ---------------------------------------------------------------------------
// Le fragment du QR
// ---------------------------------------------------------------------------

/// Ce que le fragment du QR porte (PHASE-3, § 2.6) :
/// `#<secret d'entrée, 16 octets>.<clé publique de l'autorité, 32 octets>`, en base64url
/// sans remplissage. Un navigateur n'envoie jamais le fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// Le secret d'entrée.
    pub secret: SecretDEntree,
    /// La clé publique de l'autorité.
    pub autorite: ClePublique,
}

/// Un fragment qui ne se lit pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentIllisible {
    /// Pas deux parties séparées d'un point, ou pas de la bonne longueur.
    Forme,
    /// Un caractère hors de l'alphabet base64url, ou des bits de fin non nuls.
    Base64,
}

impl Fragment {
    /// Le fragment, sans le `#` : 22 caractères, un point, 43 caractères.
    pub fn ecrire(&self) -> String {
        let mut s = base64url(self.secret.octets());
        s.push('.');
        s.push_str(&base64url(&self.autorite.0));
        s
    }

    /// Lit un fragment, sans le `#`. Une seule écriture par fragment : aucun
    /// remplissage, aucun bit de fin non nul (SAL-119).
    pub fn lire(texte: &str) -> Result<Fragment, FragmentIllisible> {
        let (secret, cle) = texte.split_once('.').ok_or(FragmentIllisible::Forme)?;
        let secret: [u8; 16] = lire_base64url(secret)?;
        let cle: [u8; 32] = lire_base64url(cle)?;
        Ok(Fragment {
            secret: SecretDEntree(secret),
            autorite: ClePublique(cle),
        })
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64url(octets: &[u8]) -> String {
    let mut texte = String::new();
    for bloc in octets.chunks(3) {
        let mut trois = [0u8; 4];
        for (place, &octet) in trois.iter_mut().skip(1).zip(bloc) {
            *place = octet;
        }
        let valeur = u32::from_be_bytes(trois);
        let caracteres: u32 = match bloc.len() {
            1 => 2,
            2 => 3,
            _ => 4,
        };
        for rang in 0..caracteres {
            let decalage = 18u32.saturating_sub(rang.saturating_mul(6));
            let indice = usize::try_from(valeur.checked_shr(decalage).unwrap_or(0) & 0x3f);
            if let Some(&caractere) = indice.ok().and_then(|i| ALPHABET.get(i)) {
                texte.push(char::from(caractere));
            }
        }
    }
    texte
}

fn valeur_base64url(c: u8) -> Option<u32> {
    let position = ALPHABET.iter().position(|&a| a == c)?;
    u32::try_from(position).ok()
}

fn lire_base64url<const N: usize>(texte: &str) -> Result<[u8; N], FragmentIllisible> {
    // N octets s'écrivent en ⌈4N/3⌉ caractères, sans remplissage.
    let attendus = N.saturating_mul(4).div_ceil(3);
    if texte.len() != attendus {
        return Err(FragmentIllisible::Forme);
    }
    let mut sortie = [0u8; N];
    let mut ecrits = 0usize;
    for bloc in texte.as_bytes().chunks(4) {
        let mut n: u32 = 0;
        for (rang, &c) in bloc.iter().enumerate() {
            let v = valeur_base64url(c).ok_or(FragmentIllisible::Base64)?;
            let rang = u32::try_from(rang).map_err(|_| FragmentIllisible::Forme)?;
            let decalage = 18u32.saturating_sub(rang.saturating_mul(6));
            n |= v.checked_shl(decalage).unwrap_or(0);
        }
        let octets = match bloc.len() {
            2 => 1,
            3 => 2,
            4 => 3,
            _ => return Err(FragmentIllisible::Forme),
        };
        let trois = n.to_be_bytes();
        let utiles = trois.get(1..).unwrap_or(&[]);
        // Les bits qui ne portent aucun octet doivent être nuls : une seule écriture.
        if utiles.iter().skip(octets).any(|&b| b != 0) {
            return Err(FragmentIllisible::Base64);
        }
        for &b in utiles.iter().take(octets) {
            let place = sortie.get_mut(ecrits).ok_or(FragmentIllisible::Forme)?;
            *place = b;
            ecrits = ecrits.saturating_add(1);
        }
    }
    if ecrits != N {
        return Err(FragmentIllisible::Forme);
    }
    Ok(sortie)
}

// ---------------------------------------------------------------------------
// Les erreurs
// ---------------------------------------------------------------------------

/// Ce qui fait refuser un message chiffré. Une trame refusée ne coûte que son lien
/// (SAL-117) ; aucune ne fait paniquer (REM-64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurChiffre {
    /// Un message de poignée de main qui ne se déchiffre pas (un autre secret d'entrée,
    /// une autre autorité, des octets abîmés), ou qui n'a pas exactement
    /// [`TAILLE_POIGNEE`] octets : la poignée de main ne porte rien (SAL-17).
    PoigneeIllisible,
    /// Une trame plus courte que son nonce et son étiquette.
    TrameTropCourte,
    /// Une trame au-delà de [`TRAME_MAX`].
    TrameTropLongue,
    /// Un clair au-delà de [`CLAIR_MAX`].
    ClairTropLong,
    /// Une trame qui ne se déchiffre pas : abîmée, d'une autre session, ou d'une autre
    /// classe que celle que dit le relais.
    Dechiffrement,
    /// Une trame déjà reçue, rejouée.
    Rejouee,
    /// Une trame plus vieille que la fenêtre de rejeu.
    HorsFenetre,
    /// Tous les nonces d'une session sont pris : il faut une poignée de main neuve.
    NoncesEpuises,
    /// Une case de la garde trop courte, abîmée, ou lue sous un autre en-tête.
    CaseIllisible,
    /// Une erreur de `snow` qui ne vient pas des octets reçus.
    Interne,
}

// ---------------------------------------------------------------------------
// La poignée de main
// ---------------------------------------------------------------------------

/// Le résolveur que `snow` reçoit : X25519, `ChaChaPoly` et `BLAKE2s` par celui de `snow`,
/// et un hasard qui ne sert qu'une fois les 32 octets d'une clé, tirés du [`Hasard`] de
/// `salle`.
struct Resolveur {
    cle: [u8; 32],
}

impl Resolveur {
    fn new(cle: [u8; 32]) -> Resolveur {
        Resolveur { cle }
    }
}

impl CryptoResolver for Resolveur {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        Some(Box::new(UneCle(Some(self.cle))))
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        DefaultResolver.resolve_dh(choice)
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        DefaultResolver.resolve_hash(choice)
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        DefaultResolver.resolve_cipher(choice)
    }
}

/// Un hasard qui sert une clé de 32 octets, une seule fois.
struct UneCle(Option<[u8; 32]>);

impl Random for UneCle {
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), snow::Error> {
        match (self.0.take(), dest.len()) {
            (Some(cle), 32) => {
                dest.copy_from_slice(&cle);
                Ok(())
            }
            (Some(_) | None, _) => Err(snow::Error::Rng),
        }
    }
}

fn parametres() -> Result<NoiseParams, ErreurChiffre> {
    MOTIF.parse().map_err(|_| ErreurChiffre::Interne)
}

/// Qui commence la poignée de main.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cote {
    /// L'appareil.
    Initiateur,
    /// L'autorité.
    Repondeur,
}

/// Un état de poignée de main, d'un côté. `cle` est la clé publique de l'autorité pour
/// l'initiateur, sa clé privée pour le répondeur ; `ephemere`, la clé éphémère.
fn construire(
    cote: Cote,
    prologue: &[u8],
    psk: &[u8; 32],
    cle: &[u8; 32],
    ephemere: [u8; 32],
) -> Result<HandshakeState, ErreurChiffre> {
    let b = Builder::with_resolver(parametres()?, Box::new(Resolveur::new(ephemere)))
        .prologue(prologue)
        .and_then(|b| b.psk(0, psk));
    let etat = match cote {
        Cote::Initiateur => b
            .and_then(|b| b.remote_public_key(cle))
            .and_then(Builder::build_initiator),
        Cote::Repondeur => b
            .and_then(|b| b.local_private_key(cle))
            .and_then(Builder::build_responder),
    };
    etat.map_err(|_| ErreurChiffre::Interne)
}

/// Écrit un message de poignée de main qui porte `charge`.
fn ecrire_poignee(etat: &mut HandshakeState, charge: &[u8]) -> Result<Vec<u8>, ErreurChiffre> {
    let mut message = vec![0u8; charge.len().saturating_add(TAILLE_POIGNEE)];
    let n = etat
        .write_message(charge, &mut message)
        .map_err(|_| ErreurChiffre::Interne)?;
    message.truncate(n);
    Ok(message)
}

/// Lit un message de poignée de main, et rend sa charge.
fn lire_poignee(etat: &mut HandshakeState, message: &[u8]) -> Result<Vec<u8>, ErreurChiffre> {
    let mut charge = vec![0u8; message.len()];
    let n = etat
        .read_message(message, &mut charge)
        .map_err(|_| ErreurChiffre::PoigneeIllisible)?;
    charge.truncate(n);
    Ok(charge)
}

/// La poignée de main de l'appareil, entre son premier message et la réponse de
/// l'autorité.
pub struct Initiateur {
    etat: HandshakeState,
}

impl fmt::Debug for Initiateur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Initiateur(…)")
    }
}

impl Initiateur {
    /// Commence la poignée de main : rend l'état et le premier message
    /// (`-> psk, e, es`), [`TAILLE_POIGNEE`] octets, sans charge.
    pub fn commencer(
        fragment: &Fragment,
        h: &mut (impl Hasard + ?Sized),
    ) -> Result<(Initiateur, Vec<u8>), ErreurChiffre> {
        let mut etat = construire(
            Cote::Initiateur,
            PROLOGUE,
            &fragment.secret.psk(),
            &fragment.autorite.0,
            tirer(h),
        )?;
        let premier = ecrire_poignee(&mut etat, &[])?;
        Ok((Initiateur { etat }, premier))
    }

    /// Finit la poignée de main sur la réponse de l'autorité (`<- e, ee`). Seule
    /// l'autorité qui a la clé privée du fragment sait l'écrire.
    pub fn finir(mut self, reponse: &[u8]) -> Result<Session, ErreurChiffre> {
        if reponse.len() != TAILLE_POIGNEE {
            return Err(ErreurChiffre::PoigneeIllisible);
        }
        if !lire_poignee(&mut self.etat, reponse)?.is_empty() {
            return Err(ErreurChiffre::PoigneeIllisible);
        }
        Session::apres(self.etat, Cote::Initiateur)
    }
}

/// La réponse de l'autorité au premier message d'un appareil : la session et le second
/// message (`<- e, ee`), [`TAILLE_POIGNEE`] octets, sans charge.
///
/// **Ce premier message se rejoue** : quiconque l'a capté peut le renvoyer, et obtient
/// une réponse. La session rendue n'est donc pas confirmée ([`Session::est_confirmee`])
/// tant qu'aucune trame ne s'y est ouverte : seul le détenteur de la clé éphémère sait
/// en écrire une. Rien ne doit en dépendre avant (SAL-17).
pub fn repondre(
    paire: &PaireAutorite,
    secret: &SecretDEntree,
    premier: &[u8],
    h: &mut (impl Hasard + ?Sized),
) -> Result<(Session, Vec<u8>), ErreurChiffre> {
    if premier.len() != TAILLE_POIGNEE {
        return Err(ErreurChiffre::PoigneeIllisible);
    }
    let mut etat = construire(
        Cote::Repondeur,
        PROLOGUE,
        &secret.psk(),
        &paire.privee,
        tirer(h),
    )?;
    if !lire_poignee(&mut etat, premier)?.is_empty() {
        return Err(ErreurChiffre::PoigneeIllisible);
    }
    let second = ecrire_poignee(&mut etat, &[])?;
    let session = Session::apres(etat, Cote::Repondeur)?;
    Ok((session, second))
}

// ---------------------------------------------------------------------------
// Les trames
// ---------------------------------------------------------------------------

/// La fenêtre de rejeu d'une session : le plus haut nonce reçu, et lesquels des
/// [`FENETRE_DE_REJEU`] précédents l'ont été (le bit `i` dit `haut - i`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FenetreDeRejeu {
    haut: Option<u64>,
    vus: [u64; 16],
}

impl FenetreDeRejeu {
    /// Un nonce acceptable, avant de déchiffrer : ni déjà vu, ni trop vieux.
    fn verifier(&self, n: u64) -> Result<(), ErreurChiffre> {
        let Some(haut) = self.haut else {
            return Ok(());
        };
        let Some(ecart) = haut.checked_sub(n) else {
            return Ok(());
        };
        if ecart >= FENETRE_DE_REJEU {
            return Err(ErreurChiffre::HorsFenetre);
        }
        if self.bit(ecart) {
            return Err(ErreurChiffre::Rejouee);
        }
        Ok(())
    }

    /// Note un nonce reçu, après l'avoir déchiffré.
    fn noter(&mut self, n: u64) {
        match self.haut {
            Some(haut) if n <= haut => {
                if let Some(ecart) = haut.checked_sub(n) {
                    self.poser(ecart);
                }
            }
            Some(haut) => {
                self.decaler(n.saturating_sub(haut));
                self.haut = Some(n);
                self.poser(0);
            }
            None => {
                self.haut = Some(n);
                self.poser(0);
            }
        }
    }

    fn bit(&self, ecart: u64) -> bool {
        let (mot, rang) = (ecart / 64, ecart % 64);
        usize::try_from(mot)
            .ok()
            .and_then(|m| self.vus.get(m))
            .is_some_and(|m| (m >> rang) & 1 == 1)
    }

    fn poser(&mut self, ecart: u64) {
        let (mot, rang) = (ecart / 64, ecart % 64);
        if let Some(m) = usize::try_from(mot).ok().and_then(|m| self.vus.get_mut(m)) {
            *m |= 1 << rang;
        }
    }

    /// Vieillit chaque nonce vu de `k` : le bit `i` passe en `i + k`.
    fn decaler(&mut self, k: u64) {
        if k >= FENETRE_DE_REJEU {
            self.vus = [0; 16];
            return;
        }
        let (mots, bits) = (
            usize::try_from(k / 64).unwrap_or(16),
            u32::try_from(k % 64).unwrap_or(0),
        );
        let ancien = self.vus;
        for (j, mot) in self.vus.iter_mut().enumerate() {
            let source = j.checked_sub(mots).and_then(|s| ancien.get(s)).copied();
            let voisin = j
                .checked_sub(mots)
                .and_then(|s| s.checked_sub(1))
                .and_then(|s| ancien.get(s))
                .copied();
            let haut = source.unwrap_or(0).checked_shl(bits).unwrap_or(0);
            let bas = match (bits, voisin) {
                (0, _) | (_, None) => 0,
                (b, Some(v)) => v.checked_shr(64u32.saturating_sub(b)).unwrap_or(0),
            };
            *mot = haut | bas;
        }
    }
}

/// Une session de transport entre un appareil et l'autorité, après la poignée de main.
pub struct Session {
    envoi: ChaCha20Poly1305,
    reception: ChaCha20Poly1305,
    prochain: u64,
    fenetre: FenetreDeRejeu,
    confirmee: bool,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("prochain", &self.prochain)
            .field("haut", &self.fenetre.haut)
            .field("confirmee", &self.confirmee)
            .finish_non_exhaustive()
    }
}

/// Le nonce de ChaCha20-Poly1305 de Noise : quatre octets nuls, puis le compteur en
/// petit-boutiste (la spécification de Noise, « The `ChaChaPoly` cipher functions »).
fn nonce_de_noise(n: u64) -> Nonce {
    let mut o = [0u8; 12];
    for (sortie, octet) in o.iter_mut().skip(4).zip(n.to_le_bytes()) {
        *sortie = octet;
    }
    Nonce::from(o)
}

/// Les données associées d'une trame : sa classe, encodée comme `liaison` l'encode.
fn donnees_associees(classe: Classe) -> Result<Vec<u8>, ErreurChiffre> {
    let mut v = Vec::new();
    jeu_abi::encoder(&classe, &mut v).map_err(|_| ErreurChiffre::Interne)?;
    Ok(v)
}

/// Chiffre `clair` au nonce `n` : le chiffré, puis l'étiquette.
fn chiffrer(
    aead: &ChaCha20Poly1305,
    n: u64,
    ad: &[u8],
    clair: &[u8],
) -> Result<Vec<u8>, ErreurChiffre> {
    let mut v = clair.to_vec();
    let etiquette = aead
        .encrypt_in_place_detached(&nonce_de_noise(n), ad, &mut v)
        .map_err(|_| ErreurChiffre::ClairTropLong)?;
    v.extend_from_slice(&etiquette);
    Ok(v)
}

/// Déchiffre `chiffre` (le chiffré, puis l'étiquette) au nonce `n`.
fn dechiffrer(
    aead: &ChaCha20Poly1305,
    n: u64,
    ad: &[u8],
    chiffre: &[u8],
) -> Result<Vec<u8>, ErreurChiffre> {
    let coupure = chiffre
        .len()
        .checked_sub(OCTETS_DE_L_ETIQUETTE)
        .ok_or(ErreurChiffre::TrameTropCourte)?;
    let (corps, etiquette) = chiffre
        .split_at_checked(coupure)
        .ok_or(ErreurChiffre::TrameTropCourte)?;
    let etiquette: [u8; OCTETS_DE_L_ETIQUETTE] = etiquette
        .try_into()
        .map_err(|_| ErreurChiffre::TrameTropCourte)?;
    let mut v = corps.to_vec();
    aead.decrypt_in_place_detached(&nonce_de_noise(n), ad, &mut v, &Tag::from(etiquette))
        .map_err(|_| ErreurChiffre::Dechiffrement)?;
    Ok(v)
}

impl Session {
    /// La session d'un côté, sur une poignée de main finie : les deux clés du `Split`
    /// de Noise, la première de l'appareil vers l'autorité, la seconde en retour.
    fn apres(mut etat: HandshakeState, cote: Cote) -> Result<Session, ErreurChiffre> {
        if !etat.is_handshake_finished() {
            return Err(ErreurChiffre::Interne);
        }
        let (de_l_appareil, de_l_autorite) = etat.dangerously_get_raw_split();
        let (envoi, reception) = match cote {
            Cote::Initiateur => (de_l_appareil, de_l_autorite),
            Cote::Repondeur => (de_l_autorite, de_l_appareil),
        };
        Ok(Session {
            envoi: ChaCha20Poly1305::new(&Key::from(envoi)),
            reception: ChaCha20Poly1305::new(&Key::from(reception)),
            prochain: 0,
            fenetre: FenetreDeRejeu::default(),
            // L'appareil a authentifié l'autorité en lisant sa réponse ; l'autorité
            // n'a encore rien authentifié (SAL-17).
            confirmee: cote == Cote::Initiateur,
        })
    }

    /// Une trame ouverte dans cette session : côté autorité, l'appareil a prouvé qu'il
    /// tient la clé éphémère de la poignée de main. Avant, la poignée de main peut être
    /// un rejeu, et rien ne doit en dépendre (SAL-17).
    pub const fn est_confirmee(&self) -> bool {
        self.confirmee
    }

    /// Scelle `clair` dans une trame : le nonce sur 8 octets en petit-boutiste, le
    /// chiffré, l'étiquette ; la classe en données associées.
    pub fn sceller(&mut self, classe: Classe, clair: &[u8]) -> Result<Vec<u8>, ErreurChiffre> {
        if clair.len() > CLAIR_MAX {
            return Err(ErreurChiffre::ClairTropLong);
        }
        let n = self.prochain;
        // `u64::MAX` est réservé par Noise : il ne sert jamais de nonce.
        self.prochain = n
            .checked_add(1)
            .filter(|&p| p < u64::MAX)
            .ok_or(ErreurChiffre::NoncesEpuises)?;
        let mut trame = n.to_le_bytes().to_vec();
        trame.extend(chiffrer(
            &self.envoi,
            n,
            &donnees_associees(classe)?,
            clair,
        )?);
        Ok(trame)
    }

    /// Ouvre une trame reçue, de la classe que dit le relais. Une trame abîmée, d'une
    /// autre classe, déjà reçue ou plus vieille que la fenêtre est refusée, et ne change
    /// rien à la session.
    pub fn ouvrir(&mut self, classe: Classe, trame: &[u8]) -> Result<Vec<u8>, ErreurChiffre> {
        if trame.len() < SURCOUT_TRAME {
            return Err(ErreurChiffre::TrameTropCourte);
        }
        if u32::try_from(trame.len()).map_or(true, |l| l > TRAME_MAX) {
            return Err(ErreurChiffre::TrameTropLongue);
        }
        let (nonce, chiffre) = trame
            .split_first_chunk::<OCTETS_DU_NONCE>()
            .ok_or(ErreurChiffre::TrameTropCourte)?;
        let n = u64::from_le_bytes(*nonce);
        if n == u64::MAX {
            return Err(ErreurChiffre::Dechiffrement);
        }
        self.fenetre.verifier(n)?;
        let clair = dechiffrer(&self.reception, n, &donnees_associees(classe)?, chiffre)?;
        self.fenetre.noter(n);
        self.confirmee = true;
        Ok(clair)
    }
}

// ---------------------------------------------------------------------------
// Les cases de la garde
// ---------------------------------------------------------------------------

/// Ce qui situe une case dans la chaîne de la garde, et que ses données associées
/// portent (PHASE-3, § 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnteteCase {
    /// La salle.
    pub salle: CodeSalle,
    /// La case : sa sorte et son numéro.
    pub case: Case,
    /// L'empreinte de la case précédente ([`empreinte_case`]), ou
    /// [`AVANT_LA_PREMIERE_CASE`].
    pub precedente: [u8; 32],
}

impl EnteteCase {
    /// Les données associées : `soiree case 1`, la salle (4 octets), la sorte (0 pour
    /// un point, 1 pour un journal), le numéro (8 octets en petit-boutiste),
    /// l'empreinte de la case précédente (32 octets).
    fn donnees_associees(&self) -> Vec<u8> {
        let (sorte, n) = match self.case {
            Case::Point(n) => (0u8, n),
            Case::Journal(n) => (1u8, n),
        };
        let mut v = b"soiree case 1".to_vec();
        v.extend_from_slice(&self.salle.0);
        v.push(sorte);
        v.extend_from_slice(&n.to_le_bytes());
        v.extend_from_slice(&self.precedente);
        v
    }
}

/// Scelle une case de la garde : le nonce de 24 octets, tiré ; le chiffré ;
/// l'étiquette.
pub fn sceller_case(
    g: &CleDeGarde,
    entete: &EnteteCase,
    clair: &[u8],
    h: &mut (impl Hasard + ?Sized),
) -> Result<Vec<u8>, ErreurChiffre> {
    let aead = XChaCha20Poly1305::new(&Key::from(g.cle_des_cases()));
    let nonce: [u8; OCTETS_DU_NONCE_DE_CASE] = tirer(h);
    let mut corps = clair.to_vec();
    let etiquette = aead
        .encrypt_in_place_detached(
            &XNonce::from(nonce),
            &entete.donnees_associees(),
            &mut corps,
        )
        .map_err(|_| ErreurChiffre::ClairTropLong)?;
    let mut v = nonce.to_vec();
    v.extend_from_slice(&corps);
    v.extend_from_slice(&etiquette);
    Ok(v)
}

/// Un bloc de `ChaCha20` : une tranche du scellement en est un multiple, pour que Poly1305
/// la lise comme le scellement d'un coup.
const OCTETS_D_UN_BLOC: usize = 64;

/// Une case scellée par tranches (lot 10a, correction après la vérification) : le point
/// de reprise, d'environ un Mio, se scelle sur plusieurs réveils du Worker, pour qu'aucun
/// tour ne tienne le fil plus d'un pas. Rend **exactement** les octets de
/// [`sceller_case`] (le même nonce tiré au même moment) et leur [`empreinte_case`],
/// calculée en chemin : `XChaCha20` puis Poly1305 sur chaque tranche, comme
/// `chacha20poly1305` sur tout le corps (RFC 8439, § 2.8).
pub struct ScellementDeCase {
    chacha: XChaCha20,
    mac: Poly1305,
    hacheur: blake3::Hasher,
    longueur_ad: u64,
    /// Le nonce, puis le corps, chiffré jusqu'à `fait`, puis l'étiquette à la fin.
    octets: Vec<u8>,
    fait: usize,
}

impl fmt::Debug for ScellementDeCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScellementDeCase")
            .field("octets", &self.octets.len())
            .field("fait", &self.fait)
            .finish_non_exhaustive()
    }
}

impl ScellementDeCase {
    /// Commence à sceller `clair` sous `entete` : le nonce est tiré maintenant, rien
    /// n'est encore chiffré.
    pub fn commencer(
        g: &CleDeGarde,
        entete: &EnteteCase,
        clair: &[u8],
        h: &mut (impl Hasard + ?Sized),
    ) -> Result<ScellementDeCase, ErreurChiffre> {
        // La borne de `chacha20poly1305` : le compteur de blocs ne déborde pas.
        if clair.len() / OCTETS_D_UN_BLOC >= u32::MAX as usize {
            return Err(ErreurChiffre::ClairTropLong);
        }
        let nonce: [u8; OCTETS_DU_NONCE_DE_CASE] = tirer(h);
        let mut chacha = XChaCha20::new(
            &chacha20::Key::from(g.cle_des_cases()),
            &chacha20::XNonce::from(nonce),
        );
        // La clé de Poly1305 : les 32 premiers octets du flot ; le corps commence au
        // bloc 1.
        let mut cle_mac = poly1305::Key::default();
        chacha.apply_keystream(&mut cle_mac);
        let mut mac = Poly1305::new(&cle_mac);
        cle_mac.fill(0);
        chacha.seek(OCTETS_D_UN_BLOC as u64);
        let ad = entete.donnees_associees();
        mac.update_padded(&ad);
        let mut octets = Vec::with_capacity(
            OCTETS_DU_NONCE_DE_CASE
                .saturating_add(clair.len())
                .saturating_add(OCTETS_DE_L_ETIQUETTE),
        );
        octets.extend_from_slice(&nonce);
        octets.extend_from_slice(clair);
        let mut hacheur = blake3::Hasher::new();
        hacheur.update(&nonce);
        Ok(ScellementDeCase {
            chacha,
            mac,
            hacheur,
            longueur_ad: ad.len() as u64,
            octets,
            fait: OCTETS_DU_NONCE_DE_CASE,
        })
    }

    /// Chiffre la tranche suivante, d'au plus `octets_max` octets (arrondi au bloc, au
    /// moins un bloc). Rend vrai quand tout le corps est chiffré.
    pub fn avancer(&mut self, octets_max: usize) -> bool {
        let reste = self.octets.len().saturating_sub(self.fait);
        let tranche = (octets_max / OCTETS_D_UN_BLOC)
            .max(1)
            .saturating_mul(OCTETS_D_UN_BLOC)
            .min(reste);
        let fin = self.fait.saturating_add(tranche);
        if let Some(t) = self.octets.get_mut(self.fait..fin) {
            self.chacha.apply_keystream(t);
            // Une tranche est un multiple de 16 octets, sauf la dernière : Poly1305 ne
            // complète que celle-là, comme le scellement d'un coup.
            self.mac.update_padded(t);
            self.hacheur.update(t);
        }
        self.fait = fin;
        self.fait >= self.octets.len()
    }

    /// Finit le scellement (ce qui reste se chiffre d'un coup) : la case scellée, et son
    /// empreinte.
    pub fn finir(mut self) -> (Vec<u8>, [u8; 32]) {
        while !self.avancer(usize::MAX) {}
        let longueur = self.octets.len().saturating_sub(OCTETS_DU_NONCE_DE_CASE) as u64;
        let mut bloc = poly1305::Block::default();
        let (ad, corps) = bloc.split_at_mut(8);
        ad.copy_from_slice(&self.longueur_ad.to_le_bytes());
        corps.copy_from_slice(&longueur.to_le_bytes());
        self.mac.update(&[bloc]);
        let etiquette = self.mac.finalize();
        self.hacheur.update(&etiquette);
        self.octets.extend_from_slice(&etiquette);
        (self.octets, *self.hacheur.finalize().as_bytes())
    }
}

/// Ouvre une case de la garde, sous l'en-tête où on l'attend. Une case d'une autre
/// salle, d'une autre sorte, d'un autre numéro, qui ne suit pas la case précédente, ou
/// abîmée, est refusée.
pub fn ouvrir_case(
    g: &CleDeGarde,
    entete: &EnteteCase,
    scellee: &[u8],
) -> Result<Vec<u8>, ErreurChiffre> {
    let (nonce, reste) = scellee
        .split_first_chunk::<OCTETS_DU_NONCE_DE_CASE>()
        .ok_or(ErreurChiffre::CaseIllisible)?;
    let coupure = reste
        .len()
        .checked_sub(OCTETS_DE_L_ETIQUETTE)
        .ok_or(ErreurChiffre::CaseIllisible)?;
    let (corps, etiquette) = reste
        .split_at_checked(coupure)
        .ok_or(ErreurChiffre::CaseIllisible)?;
    let etiquette: [u8; OCTETS_DE_L_ETIQUETTE] = etiquette
        .try_into()
        .map_err(|_| ErreurChiffre::CaseIllisible)?;
    let aead = XChaCha20Poly1305::new(&Key::from(g.cle_des_cases()));
    let mut v = corps.to_vec();
    aead.decrypt_in_place_detached(
        &XNonce::from(*nonce),
        &entete.donnees_associees(),
        &mut v,
        &Tag::from(etiquette),
    )
    .map_err(|_| ErreurChiffre::CaseIllisible)?;
    Ok(v)
}

/// L'empreinte d'une case scellée, que les données associées de la suivante portent :
/// `blake3` de ses octets.
pub fn empreinte_case(scellee: &[u8]) -> [u8; 32] {
    *blake3::hash(scellee).as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexa(t: &str) -> Vec<u8> {
        t.as_bytes()
            .chunks(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect()
    }

    fn hexa32(t: &str) -> [u8; 32] {
        hexa(t).try_into().unwrap()
    }

    /// Le vecteur public de `Noise_NKpsk0_25519_ChaChaPoly_BLAKE2s`, tel que les vecteurs
    /// de cacophony le donnent, et que `snow` 0.10.0 les livre
    /// (`tests/vectors/cacophony.txt`, l. 10137 à 10173) : les deux messages de la
    /// poignée de main (avec des charges, que notre poignée de main n'a pas), l'empreinte
    /// de la poignée de main, puis quatre messages de transport, deux dans chaque sens,
    /// aux nonces 0 et 1, sans données associées. Nos fonctions de poignée de main, de
    /// `Split` et de transport doivent retrouver chaque octet.
    #[test]
    fn les_vecteurs_publics_de_noise_nkpsk0_se_retrouvent() {
        let prologue = hexa("4a6f686e2047616c74");
        let psk = hexa32("54686973206973206d7920417573747269616e20706572737065637469766521");
        let init_ephemere =
            hexa32("893e28b9dc6ca8d611ab664754b8ceb7bac5117349a4439a6b0569da977c464a");
        let resp_statique =
            hexa32("4a3acbfdb163dec651dfa3194dece676d437029c62a408b4c5ea9114246e4893");
        let resp_publique =
            hexa32("31e0303fd6418d2f8c0e78b91f22e8caed0fbe48656dcf4767e4834f701b8f62");
        let resp_ephemere =
            hexa32("bbdb4cdbd309f1a1f2e1456967fe288cadd6f712d65dc7b7793d5e63da6b375b");
        let empreinte = hexa("6bd69bd4066f41f32e47134976f5bf01606f7a4a0e04369fe61158b06f3a144e");
        let messages = [
            (
                "4c756477696720766f6e204d69736573",
                "ca35def5ae56cec33dc2036731ab14896bc4c75dbb07a61f879f8e3afa4c794427635ede06947b2d3acd77a36788aaaf17e9f5a8ac252e560fb421ba161a2cf8",
            ),
            (
                "4d757272617920526f746862617264",
                "95ebc60d2b1fa672c1f46a8aa265ef51bfe38e7ccb39ec5be34069f144808843d682eb9cf4fee6816c8c8cfd34c15774321e234e3a426d7cfd3f13e5e84d04",
            ),
            (
                "462e20412e20486179656b",
                "b6645684db57679aa08f0b3352d58f32ec7f1e1a02083d5bd54277",
            ),
            (
                "4361726c204d656e676572",
                "473a9a4109eba0939e934640d318984df8d0900aa922f0195a09ad",
            ),
            (
                "4a65616e2d426170746973746520536179",
                "c8c44a16fff728f83e61272382149feadd3eb0ee1bab6313f84c72fe1581225236",
            ),
            (
                "457567656e2042f6686d20766f6e2042617765726b",
                "21354f87158ac5e357529e87e8c84cfcdb49c8a080550c8f908d05ef7ea82ca525e3d1398e",
            ),
        ];

        // La clé publique du vecteur est bien celle de sa clé privée.
        let paire = PaireAutorite::depuis_privee(resp_statique).unwrap();
        assert_eq!(paire.publique().0, resp_publique);

        let mut init = construire(
            Cote::Initiateur,
            &prologue,
            &psk,
            &resp_publique,
            init_ephemere,
        )
        .unwrap();
        let mut resp = construire(
            Cote::Repondeur,
            &prologue,
            &psk,
            &resp_statique,
            resp_ephemere,
        )
        .unwrap();

        let (charge, attendu) = messages[0];
        let m = ecrire_poignee(&mut init, &hexa(charge)).unwrap();
        assert_eq!(m, hexa(attendu), "premier message de la poignée de main");
        assert_eq!(lire_poignee(&mut resp, &m).unwrap(), hexa(charge));

        let (charge, attendu) = messages[1];
        let m = ecrire_poignee(&mut resp, &hexa(charge)).unwrap();
        assert_eq!(m, hexa(attendu), "second message de la poignée de main");
        assert_eq!(lire_poignee(&mut init, &m).unwrap(), hexa(charge));

        assert_eq!(init.get_handshake_hash(), empreinte.as_slice());
        assert_eq!(resp.get_handshake_hash(), empreinte.as_slice());

        let mut s_init = Session::apres(init, Cote::Initiateur).unwrap();
        let mut s_resp = Session::apres(resp, Cote::Repondeur).unwrap();
        for (rang, (charge, attendu)) in messages[2..].iter().enumerate() {
            let n = u64::try_from(rang / 2).unwrap();
            let (de, vers) = if rang % 2 == 0 {
                (&mut s_init, &mut s_resp)
            } else {
                (&mut s_resp, &mut s_init)
            };
            let c = chiffrer(&de.envoi, n, &[], &hexa(charge)).unwrap();
            assert_eq!(c, hexa(attendu), "message de transport {rang}");
            assert_eq!(
                dechiffrer(&vers.reception, n, &[], &c).unwrap(),
                hexa(charge)
            );
        }
    }

    /// La fenêtre contre un modèle : un nonce est accepté s'il n'a jamais été noté et
    /// qu'il est à moins de 1 024 du plus haut noté.
    #[test]
    fn la_fenetre_de_rejeu_suit_son_modele_sur_une_suite_fixe() {
        let mut f = FenetreDeRejeu::default();
        let mut vus = std::collections::BTreeSet::<u64>::new();
        let suite: Vec<u64> = (0..3000u64)
            .map(|i| (i.wrapping_mul(2_654_435_761) % 2600).saturating_sub(i % 7))
            .collect();
        for n in suite {
            let haut = vus.last().copied();
            let attendu = match haut.and_then(|h| h.checked_sub(n)) {
                Some(ecart) if ecart >= FENETRE_DE_REJEU => Err(ErreurChiffre::HorsFenetre),
                Some(_) if vus.contains(&n) => Err(ErreurChiffre::Rejouee),
                Some(_) | None => Ok(()),
            };
            assert_eq!(f.verifier(n), attendu, "nonce {n}");
            if attendu.is_ok() {
                f.noter(n);
                vus.insert(n);
            }
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(512))]

        /// La fenêtre contre son modèle, sur des suites au hasard : des nonces proches
        /// (réordonnés, répétés) et des sauts au-delà de la fenêtre, à travers les mots de
        /// 64 bits de la carte.
        #[test]
        fn la_fenetre_de_rejeu_suit_son_modele(
            pas in proptest::collection::vec((0u64..3000, 0u64..80), 1..400),
        ) {
            let mut f = FenetreDeRejeu::default();
            let mut vus = std::collections::BTreeSet::<u64>::new();
            let mut base = 0u64;
            for (saut, recul) in pas {
                base = base.saturating_add((saut % 3).saturating_mul(saut));
                let n = base.saturating_sub(recul);
                let attendu = match vus.last().and_then(|h| h.checked_sub(n)) {
                    Some(ecart) if ecart >= FENETRE_DE_REJEU => Err(ErreurChiffre::HorsFenetre),
                    Some(_) if vus.contains(&n) => Err(ErreurChiffre::Rejouee),
                    Some(_) | None => Ok(()),
                };
                proptest::prop_assert_eq!(f.verifier(n), attendu, "nonce {}", n);
                if attendu.is_ok() {
                    f.noter(n);
                    vus.insert(n);
                }
            }
        }
    }
}
