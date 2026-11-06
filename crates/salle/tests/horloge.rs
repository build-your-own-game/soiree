//! L'horloge de la salle (PHASE-3, § 2.3 et § 2.7, lot 4b) : le pas que l'autorité joue,
//! l'heure de l'autorité estimée par écho (SAL-127, SAL-128), la demi-latence mesurée par
//! l'autorité (SAL-129).

use jeu_abi::{Duree, Instant, Pas, TexteCourt};
use partie::{BUDGETS_DE_SOIREE, Ouverture};
use salle::horloge::{
    AllerRetour, ECHOS_RETENUS, Estimation, Horloge, debut_du_pas, duree_de_us,
    instant_de_la_partie,
};

fn instant(pas: u64, fraction: u16) -> Instant {
    Instant {
        pas: Pas::du_fil(pas),
        fraction,
    }
}

#[test]
fn l_instant_de_la_partie_se_compte_en_pas() {
    assert_eq!(instant_de_la_partie(0), instant(0, 0));
    assert_eq!(instant_de_la_partie(1_000_000), instant(60, 0));
    // Un demi-pas : 8 333 µs, un peu moins de la moitié de 65 536.
    assert_eq!(instant_de_la_partie(8_333), instant(0, 32_766));
    assert_eq!(instant_de_la_partie(16_667), instant(1, 1));
    assert_eq!(debut_du_pas(Pas::du_fil(60)), 1_000_000);
    assert_eq!(debut_du_pas(Pas::du_fil(1)), 16_666);
    assert_eq!(
        instant_de_la_partie(u64::MAX).pas.numero(),
        1_106_804_644_422_573
    );
}

#[test]
fn une_demi_latence_ne_se_reduit_jamais_en_se_convertissant() {
    assert_eq!(duree_de_us(0), Duree::ZERO);
    assert_eq!(duree_de_us(1), Duree::pas(1));
    assert_eq!(duree_de_us(16_666), Duree::pas(1));
    assert_eq!(duree_de_us(16_667), Duree::pas(2));
    assert_eq!(duree_de_us(1_000_000), Duree::pas(60));
}

fn ouverture(pas: u64, heure: u64) -> Ouverture {
    Ouverture {
        graine: [0; 32],
        pas,
        heure,
        salle: TexteCourt::VIDE,
        budgets: BUDGETS_DE_SOIREE,
        point_tous_les: 600,
    }
}

/// L'horloge ne dépend que de l'ouverture : une autorité qui reprend depuis la garde
/// retrouve la même, et le temps passé à recharger devient un saut (D14).
#[test]
fn l_horloge_ne_depend_que_de_l_ouverture() {
    let h = Horloge::de(&ouverture(0, 1_700_000_000_000));
    let t0 = 1_700_000_000_000_000;
    assert_eq!(h.pas(t0), Pas::ZERO);
    assert_eq!(h.pas(t0 + 1_000_000), Pas::du_fil(60));
    assert_eq!(h.pas(t0 - 1), Pas::ZERO, "avant le pas 0 : zéro");
    assert_eq!(h.partie_us(t0 + 5), 5);
    assert_eq!(h.heure_du_pas(Pas::du_fil(60)), t0 + 1_000_000);
    // Une partie ouverte au pas 600 : ce pas commence à son heure.
    let h = Horloge::de(&ouverture(600, 1_700_000_000_000));
    assert_eq!(h.pas(t0), Pas::du_fil(600));
    assert_eq!(Horloge::de(&ouverture(600, 1_700_000_000_000)), h);
}

/// SAL-127, SAL-128 : l'heure de l'autorité se garde au plus petit aller-retour des
/// derniers échos, et se mesure en continu : un vieil écho excellent finit par sortir.
#[test]
fn l_heure_de_l_autorite_se_garde_au_plus_petit_aller_retour_sal128() {
    let mut e = Estimation::default();
    assert_eq!(e.autorite_us(1_000), None);
    // L'autorité est en avance de 1 000 000 µs sur l'appareil.
    let ecart = 1_000_000;
    for (envoi, aller, retour) in [
        (0, 50_000, 50_000),
        (200_000, 5_000, 5_000),
        (400_000, 30_000, 10_000),
    ] {
        e.noter(envoi, envoi + aller + ecart, envoi + aller + retour);
    }
    // Le plus petit aller-retour (10 ms) donne l'écart exact.
    assert_eq!(e.autorite_us(1_000_000), Some(1_000_000 + ecart));
    assert_eq!(e.incertitude_us(), Some(5_000));
    // Un écho revenu avant d'être parti ne compte pas.
    e.noter(10, 0, 5);
    assert_eq!(e.incertitude_us(), Some(5_000));
    // Après assez d'échos plus lents, le bon sort de la fenêtre.
    for k in 0..ECHOS_RETENUS as u64 {
        let envoi = 1_000_000 + k * 250_000;
        e.noter(envoi, envoi + 20_000 + ecart, envoi + 40_000);
    }
    assert_eq!(e.incertitude_us(), Some(20_000));
}

/// SAL-129 : l'autorité mesure elle-même l'aller-retour ; un appareil qui tarde à
/// répondre grossit sa demi-latence, mais rien de ce qu'il dit ne la réduit.
#[test]
fn un_appareil_peut_grossir_sa_latence_pas_la_reduire_sal129() {
    let mut a = AllerRetour::default();
    assert_eq!(a.demi(), Duree::ZERO);
    a.noter(0, 40_000);
    assert_eq!(
        a.demi(),
        Duree::pas(2),
        "20 ms : deux pas, arrondis au-dessus"
    );
    a.noter(100_000, 100_000 + 200_000);
    assert_eq!(a.demi(), Duree::pas(2), "le plus petit des derniers");
    let mut lent = AllerRetour::default();
    lent.noter(0, 200_000);
    assert_eq!(lent.demi(), Duree::pas(6));
    // Un écho « revenu avant son départ » ne compte pas.
    lent.noter(500, 100);
    assert_eq!(lent.demi(), Duree::pas(6));
}
