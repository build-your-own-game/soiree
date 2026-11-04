//! Les textes (INTERFACE.md, § 2.3 et § 8.3 ; PUN-22 à PUN-26, CAT-80 à CAT-85, TL-28).

use jeu::prelude::*;
use proptest::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

jeu::limite!(Prenom = 12 Capitales);
jeu::limite!(Phrase = 80 PointsDeCode);
jeu::limite!(Court = 3 Graphemes);
jeu::limite!(Seize = 4 Utf16);

/// Des textes plutôt hostiles : lettres, accents décomposés, emoji, liants, sélecteurs,
/// contrôles, espaces de toutes sortes, invisibles.
fn texte_hostile() -> impl Strategy<Value = String> {
    let morceaux = prop::sample::select(vec![
        "a",
        "É",
        "e\u{301}",
        "ß",
        "ﬃ",
        " ",
        "  ",
        "\t",
        "\n",
        "\u{00A0}",
        "\u{200B}",
        "\u{200D}",
        "\u{FE0F}",
        "\u{FE0E}",
        "\u{202E}",
        "\u{2066}",
        "\u{FFF9}",
        "\u{0007}",
        "👩",
        "👩\u{200D}👧",
        "1\u{FE0F}\u{20E3}",
        "🇫🇷",
        "\u{0301}",
        "{joueur}",
        "<b>",
        "Z̸̢͎",
        "\u{E0041}",
        "\u{0600}",
    ]);
    prop::collection::vec(morceaux, 0..30).prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Nettoyer deux fois revient à nettoyer une fois (PUN-26, CAT-80).
    #[test]
    fn texte_nettoyer_est_idempotent(brut in texte_hostile(), autre in any::<String>()) {
        for b in [&brut, &autre] {
            let une = Texte::<Prenom>::nettoyer(b);
            let deux = Texte::<Prenom>::nettoyer(une.as_str());
            prop_assert_eq!(une.as_str(), deux.as_str());
            let une = Texte::<Phrase>::nettoyer(b);
            let deux = Texte::<Phrase>::nettoyer(une.as_str());
            prop_assert_eq!(une.as_str(), deux.as_str());
        }
    }

    /// Un texte nettoyé tient dans sa limite, coupé entre deux graphèmes : chacun de ses
    /// graphèmes, sauf le dernier, est un graphème du texte d'avant la coupe (PUN-24,
    /// CAT-83, TL-28). Le dernier peut perdre l'espace finale qu'une marque préfixée
    /// (U+0600) avait attirée dans son graphème.
    #[test]
    fn texte_coupe_entre_graphemes(brut in texte_hostile()) {
        let t = Texte::<Court>::nettoyer(&brut);
        prop_assert!(t.poids() <= 3);
        prop_assert!(t.as_str().graphemes(true).count() <= 3);
        let entier = Texte::<Phrase>::nettoyer(&brut);
        prop_assert!(entier.as_str().starts_with(t.as_str()) || t.as_str().is_empty());
        let g_entier: Vec<&str> = entier.as_str().graphemes(true).collect();
        let coupe: Vec<&str> = t.as_str().graphemes(true).collect();
        for (i, g) in coupe.iter().enumerate().take(coupe.len().saturating_sub(1)) {
            prop_assert_eq!(Some(g), g_entier.get(i));
        }
        prop_assert!(Texte::<Seize>::nettoyer(&brut).as_str().encode_utf16().count() <= 4);
    }

    /// Ce qu'on décode est déjà propre, ou refusé : une page honnête n'est jamais refusée
    /// (SAL-117).
    #[test]
    fn texte_decode_refuse_un_texte_sale(brut in texte_hostile()) {
        let mut octets = Vec::new();
        jeu_abi::encoder(&brut, &mut octets).unwrap();
        let propre = Texte::<Prenom>::nettoyer(&brut);
        if let Ok(t) = jeu_abi::decoder::<Texte<Prenom>>(&octets) {
            prop_assert_eq!(t.as_str(), brut.as_str());
        } else {
            prop_assert_ne!(propre.as_str(), brut.as_str());
        }
        let mut o = Vec::new();
        jeu_abi::encoder(&propre, &mut o).unwrap();
        prop_assert_eq!(jeu_abi::decoder::<Texte<Prenom>>(&o).unwrap(), propre);
    }

    /// La clé appliquée deux fois donne la clé (CAT-80) ; elle ne dépend pas de la forme
    /// Unicode (PUN-22).
    #[test]
    fn texte_cle_est_stable(brut in texte_hostile()) {
        let t = Texte::<Phrase>::nettoyer(&brut);
        let c = t.cle();
        prop_assert_eq!(CleTexte::de(c.as_str()), c);
    }

    /// Un suffixe tient toujours dans la limite (PUN-25).
    #[test]
    fn texte_suffixe_tient(brut in texte_hostile(), n in 0u32..1000) {
        let t = Texte::<Prenom>::nettoyer(&brut);
        let s = t.avec_suffixe(&format!(" {n}"));
        prop_assert!(s.poids() <= 12);
        prop_assert!(s.as_str().ends_with(&n.to_string()));
        // L'espace du suffixe reste, sauf devant un texte vidé (PUN-25, besoin 4).
        if s.as_str() != n.to_string() {
            prop_assert!(s.as_str().ends_with(&format!(" {n}")), "{:?}", s.as_str());
        }
        // Le résultat est déjà propre.
        prop_assert_eq!(Texte::<Prenom>::nettoyer(s.as_str()), s);
    }
}

/// PUN-25, besoin 4 des jeux : un suffixe garde l'espace qui le sépare du prénom, et le
/// prénom raccourcit pour lui faire place.
#[test]
fn texte_suffixe_garde_son_espace() {
    let m = Texte::<Prenom>::nettoyer("MMMMMMMMMMMM");
    assert_eq!(m.avec_suffixe(" 2").as_str(), "MMMMMMMMMM 2");
    assert_eq!(m.avec_suffixe(" 12").as_str(), "MMMMMMMMM 12");
    assert_eq!(m.avec_suffixe("2").as_str(), "MMMMMMMMMMM2");
    let lea = Texte::<Prenom>::nettoyer("Léa");
    assert_eq!(lea.avec_suffixe(" 2").as_str(), "Léa 2");
    assert_eq!(lea.avec_suffixe("   2").as_str(), "Léa 2");
    // Une coupe qui tombe sur une espace ne la double pas.
    let deux = Texte::<Prenom>::nettoyer("ABCDEFGHIJ KL");
    assert_eq!(deux.avec_suffixe(" 2").as_str(), "ABCDEFGHIJ 2");
    assert_eq!(
        Texte::<Prenom>::nettoyer("").avec_suffixe(" 2").as_str(),
        "2"
    );
}

/// `ß` compte deux capitales, `ﬃ` trois (PUN-24, CAT-82).
#[test]
fn texte_capitales_compte_eszett_double() {
    let t = Texte::<Prenom>::nettoyer("ßßßßßßßß");
    assert_eq!(t.as_str(), "ßßßßßß");
    assert_eq!(t.poids(), 12);
    assert_eq!(Texte::<Prenom>::nettoyer("ﬃﬃﬃﬃﬃ").as_str(), "ﬃﬃﬃﬃ");
    assert_eq!(Texte::<Prenom>::nettoyer("Léa").poids(), 3);
}

/// La clé ignore la casse, les sélecteurs et les liants ; les accents comptent (CAT-117,
/// PUN-22).
#[test]
fn texte_cle_ignore_selecteurs_et_liants() {
    let a = Texte::<Phrase>::nettoyer("❤\u{FE0F} LÉA");
    let b = Texte::<Phrase>::nettoyer("❤ léa");
    assert_eq!(a.cle(), b.cle());
    assert_eq!(
        Texte::<Phrase>::nettoyer("e\u{301}").cle(),
        Texte::<Phrase>::nettoyer("é").cle()
    );
    assert_ne!(
        Texte::<Phrase>::nettoyer("pêche").cle(),
        Texte::<Phrase>::nettoyer("péché").cle()
    );
}

/// Un invisible ne fait pas un second « Léa », un emoji garde ses liants, une lettre
/// les perd (PUN-23, PUN-26, CAT-85).
#[test]
fn texte_invisible_ne_change_aucun_nom() {
    let lea = Texte::<Prenom>::nettoyer("Léa");
    for sale in [
        "Lé\u{200B}a",
        "\u{202E}Léa",
        "Léa\u{2060}",
        "L\u{FFF9}éa",
        "  Léa  ",
        "Le\u{301}a",
        "\u{0007}Léa",
    ] {
        assert_eq!(Texte::<Prenom>::nettoyer(sale), lea, "{sale:?}");
    }
    assert_eq!(Texte::<Prenom>::nettoyer("a \u{200B} b").as_str(), "a b");
    assert_eq!(Texte::<Prenom>::nettoyer("a\u{200D}b").as_str(), "ab");
    assert_eq!(
        Texte::<Prenom>::nettoyer("👩\u{200D}👧").as_str(),
        "👩\u{200D}👧"
    );
    assert_eq!(
        Texte::<Prenom>::nettoyer("\u{200B}\u{2060}").vide(),
        Oui::Oui
    );
    // Les marques préfixées de Cf se dessinent : elles restent (PUN-23).
    assert_eq!(Texte::<Phrase>::nettoyer("\u{0600}1").as_str(), "\u{0600}1");
}

#[test]
fn texte_borne_change_de_limite_seulement_s_il_tient() {
    let t = Texte::<Phrase>::nettoyer("abc");
    assert_eq!(
        t.borne::<Court>().map(|c| c.as_str().to_owned()).as_deref(),
        Some("abc")
    );
    let long = Texte::<Phrase>::nettoyer("abcd");
    assert!(long.borne::<Court>().is_none());
}

jeu::limite!(Ascii = 10 PointsDeCode, encre jeu::Encre::Table(&[(0x20, 0x7e)]));

#[test]
fn texte_encre_dit_ce_qui_se_dessine() {
    assert_eq!(Texte::<Ascii>::nettoyer("abc").encre(), Encrable::Tout);
    assert_eq!(Texte::<Ascii>::nettoyer("abé").encre(), Encrable::Partiel);
    assert_eq!(Texte::<Ascii>::nettoyer("éé").encre(), Encrable::Rien);
    assert_eq!(Texte::<Prenom>::nettoyer("éé").encre(), Encrable::Tout);
}
