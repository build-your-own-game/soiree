//! Le chiffrement de la salle (PHASE-3, § 2.4 et § 2.6, lot 4a), éprouvé à graine fixe
//! (SAL-09) : les dérivations et les formats figés, la poignée de main, ce qu'un invité ne
//! peut pas faire, les trames que le relais rejoue, abîme ou reclasse, les cases de la
//! garde qu'il intervertit, le fragment du QR ; et, sous proptest, rien de ce qui arrive
//! du réseau ne fait paniquer (REM-64).
//!
//! Les vecteurs publics de Noise pour `Noise_NKpsk0_25519_ChaChaPoly_BLAKE2s` sont dans
//! les tests de `src/chiffre.rs` : ils touchent les fonctions internes de la poignée de
//! main et du transport.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]

use liaison::{Case, Classe, CodeSalle, TRAME_MAX};
use proptest::prelude::*;
use salle::chiffre::{
    AVANT_LA_PREMIERE_CASE, CLAIR_MAX, CleDeGarde, ClePublique, EnteteCase, ErreurChiffre,
    FENETRE_DE_REJEU, Fragment, FragmentIllisible, Hasard, Initiateur, PROLOGUE, PaireAutorite,
    SURCOUT_TRAME, SecretDEntree, Session, TAILLE_POIGNEE, empreinte_case, ouvrir_case, repondre,
    sceller_case,
};

// ---------------------------------------------------------------------------
// Le hasard des tests, à graine fixe (SAL-09)
// ---------------------------------------------------------------------------

/// Une suite d'octets tirée d'une graine par BLAKE3 en mode extensible : la même graine
/// donne les mêmes octets.
struct Graine(blake3::OutputReader);

impl Graine {
    fn new(n: u64) -> Graine {
        let mut h = blake3::Hasher::new();
        h.update(b"soiree graine de test");
        h.update(&n.to_le_bytes());
        Graine(h.finalize_xof())
    }
}

impl Hasard for Graine {
    fn remplir(&mut self, octets: &mut [u8]) {
        self.0.fill(octets);
    }
}

fn hexa(o: &[u8]) -> String {
    use std::fmt::Write;
    o.iter().fold(String::new(), |mut s, b| {
        let _: std::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

fn depuis_hexa(t: &str) -> Vec<u8> {
    t.as_bytes()
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

fn hexa32(t: &str) -> [u8; 32] {
    depuis_hexa(t).try_into().unwrap()
}

const SALLE: CodeSalle = CodeSalle([0x3f, 0xa9, 0xc2, 0xd1]);

/// Une soirée : la paire de l'autorité, le secret d'entrée et le fragment du QR.
struct Soiree {
    paire: PaireAutorite,
    secret: SecretDEntree,
    fragment: Fragment,
}

fn soiree(h: &mut Graine) -> Soiree {
    let paire = PaireAutorite::tirer(h).unwrap();
    let secret = SecretDEntree::tirer(h);
    let fragment = Fragment {
        secret: secret.clone(),
        autorite: paire.publique(),
    };
    Soiree {
        paire,
        secret,
        fragment,
    }
}

/// Une poignée de main entière : la session de l'appareil, puis celle de l'autorité.
fn relier(s: &Soiree, h: &mut Graine) -> (Session, Session) {
    let (init, premier) = Initiateur::commencer(&s.fragment, h).unwrap();
    let (autorite, second) = repondre(&s.paire, &s.secret, &premier, h).unwrap();
    let appareil = init.finir(&second).unwrap();
    (appareil, autorite)
}

// ---------------------------------------------------------------------------
// Les dérivations, figées
// ---------------------------------------------------------------------------

/// PHASE-3, § 2.6 : la psk de Noise, 32 octets, se dérive du secret d'entrée de 16
/// octets par `blake3::derive_key("soiree secret d'entree 1", secret)`. Ses octets sont
/// figés : changer la dérivation casserait tous les QR d'une soirée en cours.
#[test]
fn la_psk_derive_du_secret_d_entree_octets_figes() {
    let secret = SecretDEntree::depuis_octets(std::array::from_fn(|i| u8::try_from(i).unwrap()));
    assert_eq!(
        hexa(&secret.psk()),
        "dac0f1baa85fe772e518a4ef767bcc5f96698acfe8f86c3137382746a06d5615",
    );
    assert_eq!(
        secret.psk(),
        blake3::derive_key("soiree secret d'entree 1", secret.octets())
    );
    let autre = SecretDEntree::depuis_octets([0xff; 16]);
    assert_ne!(autre.psk(), secret.psk());
}

/// PHASE-3, § 2.7 : la preuve d'autorité, `blake3::derive_key("soiree preuve
/// d'autorite 1", G)`, figée : le relais l'enregistre et la compare d'une soirée à
/// l'autre (P41).
#[test]
fn la_preuve_d_autorite_derive_de_g_octets_figes() {
    let g = CleDeGarde::depuis_octets([0x42; 32]);
    assert_eq!(
        hexa(&g.preuve()),
        "a3dbf8135ffa65d094a14378790a01403718a26831967217fc8a2ee7eef4a3d0"
    );
    assert_eq!(
        g.preuve(),
        blake3::derive_key("soiree preuve d'autorite 1", g.octets())
    );
}

/// La clé publique de l'autorité suit X25519 : le vecteur d'Alice de la RFC 7748,
/// § 6.1.
#[test]
fn la_cle_publique_de_l_autorite_suit_x25519_rfc7748() {
    let privee = hexa32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let paire = PaireAutorite::depuis_privee(privee).unwrap();
    assert_eq!(
        paire.publique(),
        ClePublique(hexa32(
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        ))
    );
    assert_eq!(paire.privee(), &privee);
}

/// Le prologue nomme le protocole et sa version : le changer casse toute poignée de
/// main avec une page d'avant.
#[test]
fn le_prologue_est_fige() {
    assert_eq!(PROLOGUE, b"soiree fil 1");
    assert_eq!(TAILLE_POIGNEE, 48);
    assert_eq!(SURCOUT_TRAME, 24);
    assert_eq!(CLAIR_MAX, 262_120);
    assert_eq!(FENETRE_DE_REJEU, 1024);
}

// ---------------------------------------------------------------------------
// La poignée de main
// ---------------------------------------------------------------------------

#[test]
fn une_poignee_de_main_ouvre_une_session_dans_les_deux_sens() {
    let mut h = Graine::new(1);
    let s = soiree(&mut h);
    let (init, premier) = Initiateur::commencer(&s.fragment, &mut h).unwrap();
    assert_eq!(premier.len(), TAILLE_POIGNEE);
    let (mut autorite, second) = repondre(&s.paire, &s.secret, &premier, &mut h).unwrap();
    assert_eq!(second.len(), TAILLE_POIGNEE);
    assert!(
        !autorite.est_confirmee(),
        "rien avant le premier message de transport"
    );
    let mut appareil = init.finir(&second).unwrap();
    assert!(appareil.est_confirmee());

    let bonjour = b"le bonjour de l'appareil";
    let t = appareil.sceller(Classe::Fiable, bonjour).unwrap();
    assert_eq!(t.len(), bonjour.len() + SURCOUT_TRAME);
    assert_eq!(autorite.ouvrir(Classe::Fiable, &t).unwrap(), bonjour);
    assert!(autorite.est_confirmee());

    let vue = b"une vue";
    let t = autorite.sceller(Classe::Remplacable(0), vue).unwrap();
    assert_eq!(appareil.ouvrir(Classe::Remplacable(0), &t).unwrap(), vue);
}

/// À graine fixe, les deux messages de la poignée de main et la première trame de
/// chaque côté ont leurs octets figés : le prologue, la dérivation de la psk, l'ordre
/// des clés du `Split`, le nonce explicite et les données associées ne changent pas sans
/// que ce test rougisse.
#[test]
fn les_octets_d_une_poignee_de_main_et_de_ses_trames_sont_figes() {
    let mut h = Graine::new(7);
    let s = soiree(&mut h);
    assert_eq!(
        hexa(&s.paire.publique().0),
        "420a1640ef1515912594828b7d93661e7edb685f5b6fd87f4e2080f28848e257"
    );
    let (init, premier) = Initiateur::commencer(&s.fragment, &mut h).unwrap();
    assert_eq!(
        hexa(&premier),
        "9257b2a826e3fe32bd1d9c357d111058c584c9ba076824ef71abdda0420fb225956d70579e850d7a256f23646c549234"
    );
    let (mut autorite, second) = repondre(&s.paire, &s.secret, &premier, &mut h).unwrap();
    assert_eq!(
        hexa(&second),
        "d75ecc95fb0d16500f97cca02df25816cab164814893dc2dc03725120845ad0ad0da339ddd282133d3094bb3e153d7af"
    );
    let mut appareil = init.finir(&second).unwrap();
    let t = appareil.sceller(Classe::Fiable, b"bonjour").unwrap();
    assert_eq!(
        hexa(&t),
        "0000000000000000744f613236543d0715d0c7f6aca8fb0bdd7afdd19a3a47"
    );
    assert_eq!(autorite.ouvrir(Classe::Fiable, &t).unwrap(), b"bonjour");
    let t = autorite.sceller(Classe::Remplacable(2), b"vue").unwrap();
    assert_eq!(
        hexa(&t),
        "00000000000000005793dbf52c334ace348f9aa773d9c6fd684b21"
    );
    assert_eq!(appareil.ouvrir(Classe::Remplacable(2), &t).unwrap(), b"vue");
    // Le nonce explicite est en tête, en petit-boutiste : la seconde trame a le 1.
    let t = autorite.sceller(Classe::Fiable, b"").unwrap();
    assert_eq!(&t[..8], &1u64.to_le_bytes());
}

/// Un invité d'une soirée passée a un autre secret d'entrée : il n'entre plus (P12).
#[test]
fn un_secret_d_une_autre_soiree_refuse_l_entree() {
    let mut h = Graine::new(2);
    let s = soiree(&mut h);
    let ancien = Fragment {
        secret: SecretDEntree::tirer(&mut h),
        autorite: s.paire.publique(),
    };
    let (_, premier) = Initiateur::commencer(&ancien, &mut h).unwrap();
    assert_eq!(
        repondre(&s.paire, &s.secret, &premier, &mut h).unwrap_err(),
        ErreurChiffre::PoigneeIllisible
    );
}

/// P12 : un invité qui a vu le QR (il a le secret d'entrée) mais n'a pas la clé privée
/// de l'autorité ne lit pas le premier message d'un téléphone, et ne sait pas lui
/// répondre : le téléphone refuse toute réponse qui ne vient pas de l'autorité.
#[test]
fn un_invite_ne_se_fait_pas_passer_pour_l_autorite() {
    let mut h = Graine::new(3);
    let s = soiree(&mut h);
    let imposteur = PaireAutorite::tirer(&mut h).unwrap();
    let (init, premier) = Initiateur::commencer(&s.fragment, &mut h).unwrap();
    // Il ne lit pas le premier message, adressé à la clé de l'autorité.
    assert_eq!(
        repondre(&imposteur, &s.secret, &premier, &mut h).unwrap_err(),
        ErreurChiffre::PoigneeIllisible
    );
    // Il répond avec une poignée de main qu'il mène lui-même jusqu'au bout, vers lui :
    // le téléphone ne l'accepte pas.
    let faux = Fragment {
        secret: s.secret.clone(),
        autorite: imposteur.publique(),
    };
    let (_, premier_vers_lui) = Initiateur::commencer(&faux, &mut h).unwrap();
    let (_, sa_reponse) = repondre(&imposteur, &s.secret, &premier_vers_lui, &mut h).unwrap();
    assert_eq!(
        init.finir(&sa_reponse).unwrap_err(),
        ErreurChiffre::PoigneeIllisible
    );
}

/// SAL-17 : le premier message de la poignée de main se rejoue. L'autorité y répond,
/// mais la session qu'elle en tire n'est pas confirmée, et aucune trame de l'appareil
/// honnête (ni rien que le rejoueur puisse forger) ne s'y ouvre : rien ne doit dépendre
/// d'une session avant sa première trame. `salle::Autorite` (lot 4b) s'appuie sur
/// `est_confirmee` pour n'annoncer personne avant le bonjour.
#[test]
fn une_poignee_de_main_rejouee_ne_donne_aucune_session_confirmee_sal17() {
    let mut h = Graine::new(4);
    let s = soiree(&mut h);
    let (init, premier) = Initiateur::commencer(&s.fragment, &mut h).unwrap();
    let (mut vraie, second) = repondre(&s.paire, &s.secret, &premier, &mut h).unwrap();
    let mut appareil = init.finir(&second).unwrap();
    let bonjour = appareil.sceller(Classe::Fiable, b"bonjour").unwrap();

    // Un invité capte le premier message et le renvoie.
    let (mut rejouee, _) = repondre(&s.paire, &s.secret, &premier, &mut h).unwrap();
    assert!(!rejouee.est_confirmee());
    assert_eq!(
        rejouee.ouvrir(Classe::Fiable, &bonjour).unwrap_err(),
        ErreurChiffre::Dechiffrement
    );
    let mut forge = bonjour.clone();
    forge[10] ^= 1;
    assert_eq!(
        rejouee.ouvrir(Classe::Fiable, &forge).unwrap_err(),
        ErreurChiffre::Dechiffrement
    );
    assert!(!rejouee.est_confirmee());
    // La vraie session, elle, s'ouvre.
    assert_eq!(vraie.ouvrir(Classe::Fiable, &bonjour).unwrap(), b"bonjour");
    assert!(vraie.est_confirmee());
}

/// SAL-17 : la poignée de main ne porte rien. Un premier message qui porterait le jeton
/// (écrit ici directement avec `snow`, comme le ferait une page qui se trompe) est
/// refusé.
#[test]
fn une_poignee_de_main_qui_porte_le_jeton_est_refusee_sal17() {
    use snow::params::{CipherChoice, DHChoice, HashChoice};
    use snow::resolvers::{CryptoResolver, DefaultResolver};
    use snow::types::{Cipher, Dh, Hash, Random};

    struct Fixe;
    struct Octets;
    impl Random for Octets {
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), snow::Error> {
            dest.fill(9);
            Ok(())
        }
    }
    impl CryptoResolver for Fixe {
        fn resolve_rng(&self) -> Option<Box<dyn Random>> {
            Some(Box::new(Octets))
        }
        fn resolve_dh(&self, c: &DHChoice) -> Option<Box<dyn Dh>> {
            DefaultResolver.resolve_dh(c)
        }
        fn resolve_hash(&self, c: &HashChoice) -> Option<Box<dyn Hash>> {
            DefaultResolver.resolve_hash(c)
        }
        fn resolve_cipher(&self, c: &CipherChoice) -> Option<Box<dyn Cipher>> {
            DefaultResolver.resolve_cipher(c)
        }
    }

    let mut h = Graine::new(5);
    let s = soiree(&mut h);
    let psk = s.secret.psk();
    let publique = s.paire.publique().0;
    let mut init =
        snow::Builder::with_resolver(salle::chiffre::MOTIF.parse().unwrap(), Box::new(Fixe))
            .prologue(PROLOGUE)
            .unwrap()
            .psk(0, &psk)
            .unwrap()
            .remote_public_key(&publique)
            .unwrap()
            .build_initiator()
            .unwrap();
    let mut premier = vec![0u8; 200];
    let n = init.write_message(&[0x11; 32], &mut premier).unwrap();
    premier.truncate(n);
    assert_eq!(premier.len(), TAILLE_POIGNEE + 32);
    assert_eq!(
        repondre(&s.paire, &s.secret, &premier, &mut h).unwrap_err(),
        ErreurChiffre::PoigneeIllisible
    );
    // Sans charge, le même chemin passe : c'est bien la charge qui est refusée.
    let mut init =
        snow::Builder::with_resolver(salle::chiffre::MOTIF.parse().unwrap(), Box::new(Fixe))
            .prologue(PROLOGUE)
            .unwrap()
            .psk(0, &psk)
            .unwrap()
            .remote_public_key(&publique)
            .unwrap()
            .build_initiator()
            .unwrap();
    let mut vide = vec![0u8; 200];
    let n = init.write_message(&[], &mut vide).unwrap();
    vide.truncate(n);
    assert!(repondre(&s.paire, &s.secret, &vide, &mut h).is_ok());
}

// ---------------------------------------------------------------------------
// Les trames
// ---------------------------------------------------------------------------

/// PHASE-3, § 2.6 et § 8.1 : le relais garde une copie d'une trame et la renvoie. Elle
/// est refusée, comme une trame plus vieille que la fenêtre ; une trame en retard mais
/// dans la fenêtre passe une fois ; un refus ne change rien à la session.
#[test]
fn une_trame_rejouee_par_le_relais_est_refusee() {
    let mut h = Graine::new(6);
    let s = soiree(&mut h);
    let (mut appareil, mut autorite) = relier(&s, &mut h);
    let trames: Vec<Vec<u8>> = (0..1100u32)
        .map(|i| appareil.sceller(Classe::Fiable, &i.to_le_bytes()).unwrap())
        .collect();

    assert!(autorite.ouvrir(Classe::Fiable, &trames[0]).is_ok());
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trames[0]).unwrap_err(),
        ErreurChiffre::Rejouee
    );
    // En retard dans la fenêtre : la 2 passe après la 3, une fois.
    assert!(autorite.ouvrir(Classe::Fiable, &trames[3]).is_ok());
    assert!(autorite.ouvrir(Classe::Fiable, &trames[2]).is_ok());
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trames[2]).unwrap_err(),
        ErreurChiffre::Rejouee
    );
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trames[3]).unwrap_err(),
        ErreurChiffre::Rejouee
    );
    // La 1099 fait sortir de la fenêtre tout ce qui précède la 76.
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trames[1099]).unwrap(),
        1099u32.to_le_bytes()
    );
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trames[1]).unwrap_err(),
        ErreurChiffre::HorsFenetre
    );
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trames[75]).unwrap_err(),
        ErreurChiffre::HorsFenetre
    );
    assert!(autorite.ouvrir(Classe::Fiable, &trames[76]).is_ok());
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trames[76]).unwrap_err(),
        ErreurChiffre::Rejouee
    );
    // Les refus n'ont rien changé : la suivante s'ouvre.
    assert!(autorite.ouvrir(Classe::Fiable, &trames[1098]).is_ok());
}

/// PHASE-3, § 2.6 : la classe est dans les données associées. Le relais qui la change
/// (une trame fiable passée pour remplaçable, un canal pour un autre) fait refuser la
/// trame, sans la consommer : la vraie classe l'ouvre ensuite.
#[test]
fn une_classe_changee_par_le_relais_est_refusee() {
    let mut h = Graine::new(8);
    let s = soiree(&mut h);
    let (mut appareil, mut autorite) = relier(&s, &mut h);
    let t = appareil.sceller(Classe::Remplacable(1), b"tenu").unwrap();
    for fausse in [
        Classe::Fiable,
        Classe::Remplacable(0),
        Classe::Remplacable(2),
    ] {
        assert_eq!(
            autorite.ouvrir(fausse, &t).unwrap_err(),
            ErreurChiffre::Dechiffrement,
            "{fausse:?}"
        );
    }
    assert_eq!(
        autorite.ouvrir(Classe::Remplacable(1), &t).unwrap(),
        b"tenu"
    );
}

/// SAL-117 : une trame abîmée, coupée, ou d'une autre session est refusée et ne change
/// rien ; la session reste bonne.
#[test]
fn une_trame_abimee_ou_d_une_autre_session_est_refusee_sans_rien_changer() {
    let mut h = Graine::new(9);
    let s = soiree(&mut h);
    let (mut appareil, mut autorite) = relier(&s, &mut h);
    let (mut autre, _) = relier(&s, &mut h);
    let t = appareil.sceller(Classe::Fiable, b"geste").unwrap();
    for i in 0..t.len() {
        let mut abimee = t.clone();
        abimee[i] ^= 0x80;
        assert!(
            autorite.ouvrir(Classe::Fiable, &abimee).is_err(),
            "octet {i}"
        );
    }
    assert_eq!(
        autorite
            .ouvrir(Classe::Fiable, &t[..SURCOUT_TRAME - 1])
            .unwrap_err(),
        ErreurChiffre::TrameTropCourte
    );
    let d_ailleurs = autre.sceller(Classe::Fiable, b"geste").unwrap();
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &d_ailleurs).unwrap_err(),
        ErreurChiffre::Dechiffrement
    );
    assert_eq!(autorite.ouvrir(Classe::Fiable, &t).unwrap(), b"geste");
}

/// Une trame va jusqu'à `TRAME_MAX` (256 Kio), au-delà des 65 535 octets d'un message de
/// transport de Noise ; pas un octet de plus.
#[test]
fn une_trame_va_jusqu_a_trame_max() {
    let mut h = Graine::new(10);
    let s = soiree(&mut h);
    let (mut appareil, mut autorite) = relier(&s, &mut h);
    let gros = vec![0x5a; CLAIR_MAX];
    let t = appareil.sceller(Classe::Fiable, &gros).unwrap();
    assert_eq!(t.len(), usize::try_from(TRAME_MAX).unwrap());
    assert_eq!(autorite.ouvrir(Classe::Fiable, &t).unwrap(), gros);
    assert_eq!(
        appareil
            .sceller(Classe::Fiable, &vec![0; CLAIR_MAX + 1])
            .unwrap_err(),
        ErreurChiffre::ClairTropLong
    );
    let mut trop = t;
    trop.push(0);
    assert_eq!(
        autorite.ouvrir(Classe::Fiable, &trop).unwrap_err(),
        ErreurChiffre::TrameTropLongue
    );
}

// ---------------------------------------------------------------------------
// Les cases de la garde
// ---------------------------------------------------------------------------

fn entete(case: Case, precedente: [u8; 32]) -> EnteteCase {
    EnteteCase {
        salle: SALLE,
        case,
        precedente,
    }
}

#[test]
fn une_case_se_scelle_et_se_rouvre_octets_figes() {
    let mut h = Graine::new(11);
    let g = CleDeGarde::depuis_octets([0x42; 32]);
    let e = entete(Case::Point(1), AVANT_LA_PREMIERE_CASE);
    let c = sceller_case(&g, &e, b"un point", &mut h).unwrap();
    assert_eq!(
        hexa(&c),
        "e0a9e979c4b8fe01b7e910b8606bb552823eba8f639b92d158bd07a6099f3b6b42248c3a170b3116db2abe6407de2cda"
    );
    assert_eq!(ouvrir_case(&g, &e, &c).unwrap(), b"un point");
    assert_eq!(hexa(&empreinte_case(&c)), hexa(blake3::hash(&c).as_bytes()));
}

/// PHASE-3, § 2.4 : le relais ne peut ni modifier, ni intervertir, ni rejouer une case.
/// Une case lue sous un autre en-tête (une autre sorte, un autre numéro, une autre
/// salle, une autre case précédente) ou avec une autre clé de garde est refusée.
#[test]
fn une_case_de_garde_intervertie_est_refusee() {
    let mut h = Graine::new(12);
    let g = CleDeGarde::tirer(&mut h);
    let point = entete(Case::Point(3), AVANT_LA_PREMIERE_CASE);
    let p = sceller_case(&g, &point, b"point 3", &mut h).unwrap();
    let journal = entete(Case::Journal(4), empreinte_case(&p));
    let j = sceller_case(&g, &journal, b"journal 4", &mut h).unwrap();

    assert_eq!(ouvrir_case(&g, &point, &p).unwrap(), b"point 3");
    assert_eq!(ouvrir_case(&g, &journal, &j).unwrap(), b"journal 4");

    let refusees = [
        // Les deux cases interverties.
        (point, j.clone()),
        (journal, p.clone()),
        // La même case, sous une autre sorte ou un autre numéro.
        (entete(Case::Journal(3), AVANT_LA_PREMIERE_CASE), p.clone()),
        (entete(Case::Point(4), AVANT_LA_PREMIERE_CASE), p.clone()),
        // Une case qui ne suit pas la précédente (une vieille rejouée après une autre).
        (entete(Case::Journal(4), [7; 32]), j.clone()),
        // Une autre salle.
        (
            EnteteCase {
                salle: CodeSalle([0, 0, 0, 1]),
                ..point
            },
            p.clone(),
        ),
    ];
    for (e, c) in &refusees {
        assert_eq!(
            ouvrir_case(&g, e, c).unwrap_err(),
            ErreurChiffre::CaseIllisible,
            "{e:?}"
        );
    }
    let autre_g = CleDeGarde::tirer(&mut h);
    assert_eq!(
        ouvrir_case(&autre_g, &point, &p).unwrap_err(),
        ErreurChiffre::CaseIllisible
    );
    let mut abimee = p.clone();
    *abimee.last_mut().unwrap() ^= 1;
    assert_eq!(
        ouvrir_case(&g, &point, &abimee).unwrap_err(),
        ErreurChiffre::CaseIllisible
    );
    assert_eq!(
        ouvrir_case(&g, &point, &p[..39]).unwrap_err(),
        ErreurChiffre::CaseIllisible
    );
}

// ---------------------------------------------------------------------------
// Le fragment du QR
// ---------------------------------------------------------------------------

#[test]
fn le_fragment_s_ecrit_et_se_relit() {
    let f = Fragment {
        secret: SecretDEntree::depuis_octets(std::array::from_fn(|i| u8::try_from(i).unwrap())),
        autorite: ClePublique([0xfb; 32]),
    };
    let texte = f.ecrire();
    assert_eq!(
        texte,
        "AAECAwQFBgcICQoLDA0ODw.-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_s"
    );
    assert_eq!(texte.len(), 22 + 1 + 43);
    assert_eq!(Fragment::lire(&texte).unwrap(), f);
    let mut h = Graine::new(13);
    for _ in 0..64 {
        let s = soiree(&mut h);
        assert_eq!(Fragment::lire(&s.fragment.ecrire()).unwrap(), s.fragment);
    }
}

/// SAL-119 : une seule écriture par fragment. Le remplissage, les bits de fin non nuls,
/// l'alphabet de base64 ordinaire, une mauvaise longueur ou un mauvais séparateur sont
/// refusés.
#[test]
fn un_fragment_d_une_autre_forme_est_refuse_sal119() {
    let bon = Fragment {
        secret: SecretDEntree::depuis_octets([0; 16]),
        autorite: ClePublique([0; 32]),
    }
    .ecrire();
    let (secret, cle) = bon.split_once('.').unwrap();
    let cas = [
        (format!("{secret}=.{cle}"), FragmentIllisible::Forme),
        (format!("{secret}.{cle}="), FragmentIllisible::Forme),
        (format!("{secret}{cle}"), FragmentIllisible::Forme),
        (format!("{secret}.{cle}.{cle}"), FragmentIllisible::Forme),
        (format!("{}.{cle}", &secret[1..]), FragmentIllisible::Forme),
        // Le dernier caractère du secret porte quatre bits inutiles : « B » en pose un.
        (
            format!("{}B.{cle}", &secret[..21]),
            FragmentIllisible::Base64,
        ),
        // Le dernier caractère de la clé porte deux bits inutiles.
        (
            format!("{secret}.{}B", &cle[..42]),
            FragmentIllisible::Base64,
        ),
        (
            format!("+{}.{cle}", &secret[1..]),
            FragmentIllisible::Base64,
        ),
        (
            format!("/{}.{cle}", &secret[1..]),
            FragmentIllisible::Base64,
        ),
        (
            format!("{secret}.{} ", &cle[..42]),
            FragmentIllisible::Base64,
        ),
        (String::new(), FragmentIllisible::Forme),
    ];
    for (texte, attendu) in cas {
        assert_eq!(Fragment::lire(&texte), Err(attendu), "{texte:?}");
    }
}

// ---------------------------------------------------------------------------
// REM-64 : rien de ce qui arrive du réseau ne fait paniquer
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// REM-64, SAL-117 : des octets quelconques, en premier message, en réponse, en
    /// trame ou en case, sont refusés, jamais une panique ; une session ne s'ouvre que
    /// sur une vraie trame.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_le_dechiffrement_rem64(
        graine in any::<u64>(),
        octets in proptest::collection::vec(any::<u8>(), 0..160),
        longueur_de_poignee in any::<bool>(),
    ) {
        let mut h = Graine::new(graine);
        let s = soiree(&mut h);
        let mut message = octets.clone();
        if longueur_de_poignee {
            message.resize(TAILLE_POIGNEE, 0);
        }
        prop_assert!(repondre(&s.paire, &s.secret, &message, &mut h).is_err());
        let (init, _) = Initiateur::commencer(&s.fragment, &mut h).unwrap();
        prop_assert!(init.finir(&message).is_err());
        let (_, mut autorite) = relier(&s, &mut h);
        prop_assert!(autorite.ouvrir(Classe::Fiable, &octets).is_err());
        prop_assert!(!autorite.est_confirmee());
        let g = CleDeGarde::tirer(&mut h);
        let e = entete(Case::Journal(1), AVANT_LA_PREMIERE_CASE);
        prop_assert!(ouvrir_case(&g, &e, &octets).is_err());
        let _: Result<Fragment, FragmentIllisible> =
            Fragment::lire(&String::from_utf8_lossy(&octets));
    }

    /// REM-64 : une vraie trame, ou une vraie case, abîmée d'un octet, est refusée sans
    /// panique, et la vraie s'ouvre ensuite.
    #[test]
    fn une_trame_ou_une_case_abimee_est_refusee_rem64(
        graine in any::<u64>(),
        clair in proptest::collection::vec(any::<u8>(), 0..64),
        position in any::<prop::sample::Index>(),
        masque in 1u8..=255,
    ) {
        let mut h = Graine::new(graine);
        let s = soiree(&mut h);
        let (mut appareil, mut autorite) = relier(&s, &mut h);
        let t = appareil.sceller(Classe::Fiable, &clair).unwrap();
        let mut abimee = t.clone();
        let i = position.index(abimee.len());
        abimee[i] ^= masque;
        prop_assert!(autorite.ouvrir(Classe::Fiable, &abimee).is_err());
        prop_assert_eq!(autorite.ouvrir(Classe::Fiable, &t).unwrap(), clair.clone());

        let g = CleDeGarde::tirer(&mut h);
        let e = entete(Case::Point(9), [1; 32]);
        let c = sceller_case(&g, &e, &clair, &mut h).unwrap();
        let mut abimee = c.clone();
        let i = position.index(abimee.len());
        abimee[i] ^= masque;
        prop_assert!(ouvrir_case(&g, &e, &abimee).is_err());
        prop_assert_eq!(ouvrir_case(&g, &e, &c).unwrap(), clair);
    }
}
