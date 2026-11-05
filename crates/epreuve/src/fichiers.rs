//! Les fichiers du monde simulé (§ 2.10 ; CONTRAT, « Ce que le réel impose ») : le
//! sélecteur de fichiers de l'ordinateur de la soirée, et les demandes que le commun
//! exécute pour le jeu, servies avec de vrais contenus.
//!
//! - **Le sélecteur.** Un bot de l'écran apporte des fichiers (`ComposantBot::Fichiers`) :
//!   le commun les garde, chacun sous une identité neuve, et le jeu n'en reçoit que le
//!   nom, le type et la taille (`FichierRecu`, TL-14). Il en garde au plus
//!   [`GARDES_MAX`] : un fichier plus ancien est oublié, comme un navigateur qui libère
//!   ce qu'on ne lit plus, et sa lecture échoue (`Illisible`).
//! - **`Lire`.** Les octets du fichier, sa taille vérifiée avant la lecture : plus gros
//!   que `max`, `TropGros`.
//! - **`Image`.** Un vrai décodage : un PNG de 8 bits par canal, sans entrelacement, et
//!   ses CRC vérifiés. Puis le recadrage au centre, la réduction au carré demandé, et un
//!   vrai PNG en sortie. Le monde ne sait pas écrire WebP : `WebpSinonPng` donne un PNG,
//!   comme le promet son nom. Ce qui ne se décode pas est `Illisible` ; une image de
//!   plus de [`PIXELS_MAX`] pixels, ou un carré plus grand que [`CARRE_MAX`],
//!   `TropGros`.
//! - **`Rendu`.** Un vrai PNG, aux proportions de l'écran du jeu et à son échelle, de la
//!   couleur de fond de la scène : la phase 1 n'a pas de rendu, et les pixels ne sont
//!   pas ceux de la scène.
//! - **`Telecharger`.** Fait, ou refusé par le navigateur une fois sur dix.
//! - **Abîmés, trop gros, lents.** Un fichier sur huit pèse plus de 256 Kio (l'original
//!   suivi de zéros : un PNG se décode encore, une sauvegarde ne se lit plus) ; dans
//!   la soirée hostile (E13), chaque fichier peut perdre sa fin, changer un octet ou
//!   tenir en moins de quatre octets ; et chaque demande prend son temps, jusqu'à
//!   quinze secondes pour une image.
//!
//! Ces proportions et ces durées sont des points de départ, non mesurés.

use jeu_abi::{EchecDemande, FichierRecu, IdFichier, Rgba, TexteCourt};
use partie::{DemandeArrivee, ReponseArrivee};

use crate::tirage::Tirage;

/// Les fichiers que le sélecteur garde au plus.
pub(crate) const GARDES_MAX: usize = 32;
/// Le plus de pixels d'une image que le monde décode : au-delà, `TropGros` (une bombe
/// de décompression ne passe pas).
pub(crate) const PIXELS_MAX: u64 = 16 << 20;
/// Le plus grand carré que le monde produit.
pub(crate) const CARRE_MAX: u16 = 2048;
/// La taille d'un fichier « trop gros » : l'original, suivi de zéros jusque-là.
pub(crate) const GROS: usize = 256 << 10;
/// Le plus grand côté d'un rendu à l'échelle 1, en pixels.
const RENDU_COTE: u64 = 320;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// Un fichier gardé par le sélecteur.
#[derive(Debug, Clone)]
struct Fichier {
    id: u64,
    octets: Vec<u8>,
}

/// Ce qu'un fichier subit en entrant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Abime {
    /// Il arrive tel quel, sauf un sur huit, trop gros.
    Non,
    /// Soirée hostile : il peut aussi perdre sa fin ou changer un octet.
    Hostile,
}

/// Le sélecteur de fichiers et les demandes de fichiers du commun simulé.
#[derive(Debug, Clone, Default)]
pub(crate) struct Fichiers {
    gardes: Vec<Fichier>,
    prochain: u64,
    /// Les fichiers reçus, abîmés, trop gros ; les demandes servies, et celles qui ont
    /// réussi : ce que le rapport et les tests lisent.
    pub(crate) compte: CompteFichiers,
}

/// Ce que le monde a fait des fichiers (pour le rapport et les tests).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CompteFichiers {
    pub(crate) recus: u64,
    pub(crate) abimes: u64,
    pub(crate) gros: u64,
    pub(crate) servies: u64,
    pub(crate) reussies: u64,
}

impl Fichiers {
    /// Un fichier choisi : gardé, peut-être abîmé, et ce que le jeu en reçoit.
    pub(crate) fn recevoir(
        &mut self,
        nom: &str,
        genre: &str,
        octets: Vec<u8>,
        abime: Abime,
        t: &mut Tirage,
    ) -> FichierRecu {
        self.prochain = self.prochain.saturating_add(1);
        let id = self.prochain;
        let mut octets = octets;
        if abime == Abime::Hostile && t.une_sur(2) {
            self.compte.abimes = self.compte.abimes.saturating_add(1);
            abimer(&mut octets, t);
        } else if t.une_sur(8) {
            self.compte.gros = self.compte.gros.saturating_add(1);
            octets.resize(octets.len().max(GROS), 0);
        }
        self.compte.recus = self.compte.recus.saturating_add(1);
        let taille = u64::try_from(octets.len()).unwrap_or(u64::MAX);
        self.gardes.push(Fichier { id, octets });
        if self.gardes.len() > GARDES_MAX {
            let _: Fichier = self.gardes.remove(0);
        }
        FichierRecu {
            id: IdFichier::du_fil(id),
            nom: texte_court(nom),
            genre: texte_court(genre),
            taille,
        }
    }

    fn octets(&self, f: IdFichier) -> Option<&[u8]> {
        self.gardes
            .iter()
            .find(|g| g.id == f.fil())
            .map(|g| g.octets.as_slice())
    }

    /// Le temps que prend une demande, en pas : une image jusqu'à quinze secondes, une
    /// fois sur dix au-delà de cinq.
    pub(crate) fn delai(d: &DemandeArrivee, t: &mut Tirage) -> u64 {
        match d {
            DemandeArrivee::Image { .. } if t.une_sur(10) => t.entre(300, 900),
            DemandeArrivee::Image { .. } => t.entre(6, 180),
            DemandeArrivee::Rendu { .. } => t.entre(30, 240),
            DemandeArrivee::Lire { .. } => t.entre(2, 30),
            DemandeArrivee::Telecharger { .. } => t.entre(6, 60),
            DemandeArrivee::Ligne { .. } => t.entre(12, 120),
        }
    }

    /// La réponse à une demande, à son arrivée. `ecran` : les proportions de l'écran
    /// du jeu, pour un rendu.
    pub(crate) fn servir(
        &mut self,
        d: &DemandeArrivee,
        ecran: (u64, u64),
        t: &mut Tirage,
    ) -> ReponseArrivee {
        let r = match d {
            DemandeArrivee::Image { fichier, carre, .. } => ReponseArrivee::Image(
                self.octets(*fichier)
                    .ok_or(EchecDemande::Illisible)
                    .and_then(|o| image_au_carre(o, *carre)),
            ),
            DemandeArrivee::Rendu { scene, echelle, .. } => {
                if t.une_sur(20) {
                    // Le contexte du GPU perdu pendant le rendu.
                    ReponseArrivee::Rendu(Err(EchecDemande::Interne))
                } else {
                    let (l, h) = taille_du_rendu(ecran, *echelle);
                    let fond = scene.fond.unwrap_or(Rgba::NOIR);
                    let px = [fond.0, fond.1, fond.2, fond.3];
                    ReponseArrivee::Rendu(
                        png_de(l, h, &uni(l, h, px)).ok_or(EchecDemande::TropGros),
                    )
                }
            }
            DemandeArrivee::Lire { fichier, max } => {
                ReponseArrivee::Lu(match self.octets(*fichier) {
                    None => Err(EchecDemande::Illisible),
                    Some(o) if u64::try_from(o.len()).unwrap_or(u64::MAX) > *max => {
                        Err(EchecDemande::TropGros)
                    }
                    Some(o) => Ok(o.to_vec()),
                })
            }
            DemandeArrivee::Telecharger { .. } => ReponseArrivee::Fait(if t.une_sur(10) {
                Err(EchecDemande::Refuse)
            } else {
                Ok(())
            }),
            // Une ligne sur dix échoue : un échec revient comme une réponse (SQ-78).
            DemandeArrivee::Ligne { .. } => ReponseArrivee::Fait(if t.une_sur(10) {
                Err(EchecDemande::Interne)
            } else {
                Ok(())
            }),
        };
        self.compte.servies = self.compte.servies.saturating_add(1);
        let reussie = match &r {
            ReponseArrivee::Image(x) | ReponseArrivee::Rendu(x) | ReponseArrivee::Lu(x) => {
                x.is_ok()
            }
            ReponseArrivee::Fait(x) => x.is_ok(),
        };
        if reussie {
            self.compte.reussies = self.compte.reussies.saturating_add(1);
        }
        r
    }
}

/// Un texte court tiré d'un texte quelconque : les caractères de contrôle remplacés,
/// coupé à 64 octets sur une frontière de caractère.
fn texte_court(s: &str) -> TexteCourt {
    let mut propre = String::new();
    for c in s.chars() {
        let c = if c.is_control() { '_' } else { c };
        if propre.len().saturating_add(c.len_utf8()) > TexteCourt::MAX {
            break;
        }
        propre.push(c);
    }
    TexteCourt::essayer(&propre).unwrap_or(TexteCourt::VIDE)
}

/// Un fichier abîmé (soirée hostile) : sa fin perdue, un octet changé, ou presque
/// rien.
fn abimer(o: &mut Vec<u8>, t: &mut Tirage) {
    let n = u64::try_from(o.len()).unwrap_or(0);
    match t.sous(3) {
        0 => o.truncate(usize::try_from(t.sous(n.max(1))).unwrap_or(0)),
        1 => {
            let k = usize::try_from(t.sous(n.max(1))).unwrap_or(0);
            if let Some(b) = o.get_mut(k) {
                *b ^= 0x5a;
            } else {
                o.push(0x5a);
            }
        }
        _ => o.truncate(usize::try_from(t.sous(4)).unwrap_or(0)),
    }
}

/// La taille d'un rendu : le plus grand côté à [`RENDU_COTE`] pixels, aux proportions
/// de l'écran, fois l'échelle (de 1 à 4).
fn taille_du_rendu((largeur, hauteur): (u64, u64), echelle: u8) -> (u32, u32) {
    let (l, h) = (largeur.max(1), hauteur.max(1));
    let e = u64::from(echelle.clamp(1, 4));
    let (bl, bh) = if l >= h {
        (
            RENDU_COTE,
            RENDU_COTE.saturating_mul(h).checked_div(l).unwrap_or(1),
        )
    } else {
        (
            RENDU_COTE.saturating_mul(l).checked_div(h).unwrap_or(1),
            RENDU_COTE,
        )
    };
    let c = |x: u64| u32::try_from(x.max(1).saturating_mul(e)).unwrap_or(u32::MAX);
    (c(bl), c(bh))
}

/// Une image unie, en RGBA.
fn uni(l: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
    let n = usize::try_from(u64::from(l).saturating_mul(u64::from(h))).unwrap_or(0);
    let mut o = Vec::with_capacity(n.saturating_mul(4));
    for _ in 0..n {
        o.extend_from_slice(&px);
    }
    o
}

/// Une image décodée, en RGBA de 8 bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) largeur: u32,
    pub(crate) hauteur: u32,
    pub(crate) rgba: Vec<u8>,
}

/// Décode, recadre au centre, réduit au carré, encode (TL-60).
fn image_au_carre(octets: &[u8], carre: u16) -> Result<Vec<u8>, EchecDemande> {
    if carre == 0 || carre > CARRE_MAX {
        return Err(EchecDemande::TropGros);
    }
    let i = decoder_png(octets)?;
    let cote = i.largeur.min(i.hauteur);
    let x0 = i.largeur.saturating_sub(cote) / 2;
    let y0 = i.hauteur.saturating_sub(cote) / 2;
    let c = u32::from(carre);
    let mut o =
        Vec::with_capacity(usize::try_from(c.saturating_mul(c).saturating_mul(4)).unwrap_or(0));
    for y in 0..c {
        let sy = y0.saturating_add(mise_a_l_echelle(y, c, cote));
        for x in 0..c {
            let sx = x0.saturating_add(mise_a_l_echelle(x, c, cote));
            let k = usize::try_from(
                u64::from(sy)
                    .saturating_mul(u64::from(i.largeur))
                    .saturating_add(u64::from(sx))
                    .saturating_mul(4),
            )
            .unwrap_or(usize::MAX);
            match i.rgba.get(k..k.saturating_add(4)) {
                Some(px) => o.extend_from_slice(px),
                None => return Err(EchecDemande::Interne),
            }
        }
    }
    png_de(c, c, &o).ok_or(EchecDemande::TropGros)
}

/// Le pixel source du pixel `x` d'un côté de `vers` pixels, pris dans `depuis`.
fn mise_a_l_echelle(x: u32, vers: u32, depuis: u32) -> u32 {
    let v = u64::from(x)
        .saturating_mul(u64::from(depuis))
        .checked_div(u64::from(vers))
        .unwrap_or(0);
    u32::try_from(v).unwrap_or(0).min(depuis.saturating_sub(1))
}

/// Le CRC-32 des PNG (ISO 3309).
pub(crate) fn crc32(morceaux: &[&[u8]]) -> u32 {
    let mut c: u32 = 0xffff_ffff;
    for m in morceaux {
        for &b in *m {
            c ^= u32::from(b);
            for _ in 0..8 {
                let masque = (c & 1).wrapping_neg();
                c = (c >> 1) ^ (0xedb8_8320 & masque);
            }
        }
    }
    !c
}

fn morceau(o: &mut Vec<u8>, sorte: [u8; 4], donnees: &[u8]) -> Option<()> {
    o.extend_from_slice(&u32::try_from(donnees.len()).ok()?.to_be_bytes());
    o.extend_from_slice(&sorte);
    o.extend_from_slice(donnees);
    o.extend_from_slice(&crc32(&[&sorte, donnees]).to_be_bytes());
    Some(())
}

/// Un PNG RGBA de 8 bits, sans filtre. `None` si l'image ne tient pas.
pub(crate) fn png_de(largeur: u32, hauteur: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let ligne = usize::try_from(largeur).ok()?.checked_mul(4)?;
    if rgba.len() != ligne.checked_mul(usize::try_from(hauteur).ok()?)? {
        return None;
    }
    let mut brut = Vec::with_capacity(rgba.len().saturating_add(usize::try_from(hauteur).ok()?));
    for l in rgba.chunks(ligne.max(1)) {
        brut.push(0);
        brut.extend_from_slice(l);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&largeur.to_be_bytes());
    ihdr.extend_from_slice(&hauteur.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut o = Vec::from(SIGNATURE);
    morceau(&mut o, *b"IHDR", &ihdr)?;
    morceau(
        &mut o,
        *b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&brut, 1),
    )?;
    morceau(&mut o, *b"IEND", &[])?;
    Some(o)
}

fn be32(o: &[u8], k: usize) -> Option<u32> {
    let b: [u8; 4] = o.get(k..k.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(b))
}

/// L'en-tête d'un PNG : largeur, hauteur, canaux.
type Entete = (u32, u32, u8);

/// Les morceaux d'un PNG, CRC vérifiés : son en-tête et ses données compressées.
fn morceaux(octets: &[u8]) -> Result<(Entete, Vec<u8>), EchecDemande> {
    let illisible = EchecDemande::Illisible;
    if octets.get(..8) != Some(&SIGNATURE[..]) {
        return Err(illisible);
    }
    let mut ici = 8usize;
    let mut entete: Option<Entete> = None;
    let mut idat = Vec::new();
    loop {
        let n = usize::try_from(be32(octets, ici).ok_or(illisible)?).map_err(|_| illisible)?;
        let debut = ici.checked_add(8).ok_or(illisible)?;
        let sorte = octets
            .get(ici.checked_add(4).ok_or(illisible)?..debut)
            .ok_or(illisible)?;
        let fin = debut.checked_add(n).ok_or(illisible)?;
        let donnees = octets.get(debut..fin).ok_or(illisible)?;
        if crc32(&[sorte, donnees]) != be32(octets, fin).ok_or(illisible)? {
            return Err(illisible);
        }
        match sorte {
            b"IHDR" => entete = Some(lire_entete(donnees)?),
            b"IDAT" => idat.extend_from_slice(donnees),
            b"IEND" => break,
            _ => {}
        }
        ici = fin.checked_add(4).ok_or(illisible)?;
    }
    Ok((entete.ok_or(illisible)?, idat))
}

fn lire_entete(donnees: &[u8]) -> Result<Entete, EchecDemande> {
    let illisible = EchecDemande::Illisible;
    let largeur = be32(donnees, 0).ok_or(illisible)?;
    let hauteur = be32(donnees, 4).ok_or(illisible)?;
    let profondeur = donnees.get(8).copied().ok_or(illisible)?;
    let couleur = donnees.get(9).copied().ok_or(illisible)?;
    let entrelace = donnees.get(12).copied().ok_or(illisible)?;
    let canaux = match couleur {
        0 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return Err(illisible),
    };
    if profondeur != 8 || entrelace != 0 || largeur == 0 || hauteur == 0 {
        return Err(illisible);
    }
    if u64::from(largeur).saturating_mul(u64::from(hauteur)) > PIXELS_MAX {
        return Err(EchecDemande::TropGros);
    }
    Ok((largeur, hauteur, canaux))
}

/// Décode un PNG de 8 bits par canal (gris, gris et alpha, RVB, RVBA), sans
/// entrelacement, CRC vérifiés. Ce que le monde ne sait pas lire est `Illisible`.
pub(crate) fn decoder_png(octets: &[u8]) -> Result<Image, EchecDemande> {
    let illisible = EchecDemande::Illisible;
    let ((largeur, hauteur, canaux), idat) = morceaux(octets)?;
    let bpp = usize::from(canaux);
    let ligne = usize::try_from(largeur)
        .ok()
        .and_then(|l| l.checked_mul(bpp))
        .ok_or(illisible)?;
    let rangees = usize::try_from(hauteur).map_err(|_| illisible)?;
    let attendu = ligne
        .checked_add(1)
        .and_then(|x| x.checked_mul(rangees))
        .ok_or(illisible)?;
    let brut = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&idat, attendu)
        .map_err(|_| illisible)?;
    if brut.len() != attendu {
        return Err(illisible);
    }
    let mut pixels = Vec::with_capacity(ligne.saturating_mul(rangees));
    let mut dessus = vec![0u8; ligne];
    for rangee in brut.chunks(ligne.saturating_add(1)) {
        let (&filtre, source) = rangee.split_first().ok_or(illisible)?;
        let cette = defiltrer(filtre, source, &dessus, bpp)?;
        pixels.extend_from_slice(&cette);
        dessus = cette;
    }
    let mut rgba = Vec::with_capacity(pixels.len().saturating_mul(4).checked_div(bpp).unwrap_or(0));
    for px in pixels.chunks(bpp) {
        match *px {
            [g] => rgba.extend_from_slice(&[g, g, g, 255]),
            [g, al] => rgba.extend_from_slice(&[g, g, g, al]),
            [ro, ve, bl] => rgba.extend_from_slice(&[ro, ve, bl, 255]),
            [ro, ve, bl, al] => rgba.extend_from_slice(&[ro, ve, bl, al]),
            _ => return Err(illisible),
        }
    }
    Ok(Image {
        largeur,
        hauteur,
        rgba,
    })
}

/// Une rangée défiltrée (PNG, § 9) : `dessus` est la rangée d'avant, défiltrée.
fn defiltrer(
    filtre: u8,
    source: &[u8],
    dessus: &[u8],
    bpp: usize,
) -> Result<Vec<u8>, EchecDemande> {
    let mut cette: Vec<u8> = Vec::with_capacity(source.len());
    for (x, &brut_x) in source.iter().enumerate() {
        let avant = x.checked_sub(bpp);
        let gauche = avant.and_then(|k| cette.get(k)).copied().unwrap_or(0);
        let haut = dessus.get(x).copied().unwrap_or(0);
        let coin = avant.and_then(|k| dessus.get(k)).copied().unwrap_or(0);
        let p = match filtre {
            0 => 0,
            1 => gauche,
            2 => haut,
            3 => u8::try_from(u16::midpoint(u16::from(gauche), u16::from(haut))).unwrap_or(0),
            4 => paeth(gauche, haut, coin),
            _ => return Err(EchecDemande::Illisible),
        };
        cette.push(brut_x.wrapping_add(p));
    }
    Ok(cette)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
    let p = ia.saturating_add(ib).saturating_sub(ic);
    let (pa, pb, pc) = (
        p.saturating_sub(ia).abs(),
        p.saturating_sub(ib).abs(),
        p.saturating_sub(ic).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use jeu_abi::{EchecDemande, FormatImage, IdFichier, SceneFil};
    use partie::{DemandeArrivee, ReponseArrivee};

    use super::{Abime, Fichiers, GROS, decoder_png, png_de};
    use crate::tirage::Tirage;

    /// Un dégradé : chaque pixel dit où il est.
    fn degrade(l: u32, h: u32) -> Vec<u8> {
        let mut o = Vec::new();
        for y in 0..h {
            for x in 0..l {
                o.extend_from_slice(&[
                    u8::try_from(x % 256).unwrap(),
                    u8::try_from(y % 256).unwrap(),
                    7,
                    255,
                ]);
            }
        }
        o
    }

    #[test]
    fn un_png_ecrit_se_relit() {
        let px = degrade(37, 11);
        let png = png_de(37, 11, &px).unwrap();
        let i = decoder_png(&png).unwrap();
        assert_eq!((i.largeur, i.hauteur), (37, 11));
        assert_eq!(i.rgba, px);
    }

    /// Le PNG qu'un bot écrit (`jeu::fichiers::png_uni`) se décode ici comme une photo :
    /// une soirée peut jouer une image lue jusqu'au bout. Plus de 64 Kio de pixels : il
    /// faut plusieurs blocs.
    #[test]
    fn le_png_d_un_bot_se_decode() {
        for (l, h) in [(1u16, 1u16), (3, 2), (200, 100)] {
            let o = jeu::fichiers::png_uni(l, h, jeu_abi::Rgba(9, 8, 7, 200));
            let i = decoder_png(&o).unwrap();
            assert_eq!((i.largeur, i.hauteur), (u32::from(l), u32::from(h)));
            assert!(i.rgba.chunks(4).all(|p| p == [9, 8, 7, 200]));
        }
    }

    /// Un octet changé, une fin perdue, autre chose qu'un PNG : `Illisible`, jamais une
    /// panique.
    #[test]
    fn un_png_abime_est_illisible() {
        let png = png_de(8, 8, &degrade(8, 8)).unwrap();
        for k in 0..png.len() {
            let mut a = png.clone();
            a[k] ^= 0x10;
            assert!(decoder_png(&a).is_err(), "octet {k} changé");
            assert!(decoder_png(&png[..k]).is_err(), "coupé à {k}");
        }
        assert_eq!(decoder_png(b"pas une image"), Err(EchecDemande::Illisible));
    }

    /// TL-60 : le monde sert une vraie image, recadrée au centre et réduite au carré ;
    /// une sauvegarde se lit, trop grosse elle ne se lit pas ; un fichier oublié ou
    /// abîmé ne se lit pas.
    #[test]
    fn le_monde_sert_de_vrais_contenus() {
        let mut hasard = Tirage::new(&[3; 32], "fichiers");
        let mut selecteur = Fichiers::default();
        // Une image de 40 × 20 : le carré du centre commence à x = 10.
        let png = png_de(40, 20, &degrade(40, 20)).unwrap();
        let mut ok = 0;
        let mut gros = 0;
        for _ in 0..40 {
            let recu = selecteur.recevoir(
                "photo.png",
                "image/png",
                png.clone(),
                Abime::Non,
                &mut hasard,
            );
            let reponse = selecteur.servir(
                &DemandeArrivee::Image {
                    fichier: recu.id,
                    carre: 10,
                    format: FormatImage::WebpSinonPng { qualite: 80 },
                },
                (16, 9),
                &mut hasard,
            );
            let ReponseArrivee::Image(Ok(sortie)) = reponse else {
                panic!("{reponse:?}");
            };
            let image = decoder_png(&sortie).unwrap();
            assert_eq!((image.largeur, image.hauteur), (10, 10));
            assert_eq!(image.rgba[0], 10, "recadrée au centre");
            assert_eq!(image.rgba[(9 * 10 + 9) * 4], 10 + 18, "réduite de moitié");
            let lu = selecteur.servir(
                &DemandeArrivee::Lire {
                    fichier: recu.id,
                    max: 1 << 16,
                },
                (16, 9),
                &mut hasard,
            );
            match lu {
                ReponseArrivee::Lu(Ok(o)) => {
                    assert_eq!(o, png);
                    ok += 1;
                }
                ReponseArrivee::Lu(Err(EchecDemande::TropGros)) => {
                    assert!(recu.taille >= u64::try_from(GROS).unwrap());
                    gros += 1;
                }
                autre @ (ReponseArrivee::Lu(Err(_))
                | ReponseArrivee::Image(_)
                | ReponseArrivee::Rendu(_)
                | ReponseArrivee::Fait(_)) => panic!("{autre:?}"),
            }
        }
        assert!(ok > 0 && gros > 0, "{ok} lus, {gros} trop gros");
        assert_eq!(
            selecteur.servir(
                &DemandeArrivee::Lire {
                    fichier: IdFichier::du_fil(1),
                    max: u64::MAX
                },
                (16, 9),
                &mut hasard
            ),
            ReponseArrivee::Lu(Err(EchecDemande::Illisible)),
            "le premier est oublié"
        );
        let mut abimes = 0;
        for _ in 0..40 {
            let recu = selecteur.recevoir(
                "photo.png",
                "image/png",
                png.clone(),
                Abime::Hostile,
                &mut hasard,
            );
            if let ReponseArrivee::Lu(Ok(o)) = selecteur.servir(
                &DemandeArrivee::Lire {
                    fichier: recu.id,
                    max: u64::MAX,
                },
                (16, 9),
                &mut hasard,
            ) && o != png
                && o.len() < GROS
            {
                abimes += 1;
            }
        }
        assert!(abimes > 0, "la soirée hostile abîme des fichiers");
    }

    /// TL-57 : un rendu est un vrai PNG, aux proportions de l'écran et à son échelle ;
    /// le contexte du GPU se perd parfois.
    #[test]
    fn un_rendu_est_un_vrai_png() {
        let mut selecteur = Fichiers::default();
        let mut hasard = Tirage::new(&[1; 32], "rendu");
        let mut perdus = 0;
        for _ in 0..40 {
            match selecteur.servir(
                &DemandeArrivee::Rendu {
                    scene: SceneFil::vide(),
                    echelle: 2,
                    format: FormatImage::Png,
                },
                (16, 9),
                &mut hasard,
            ) {
                ReponseArrivee::Rendu(Ok(o)) => {
                    let image = decoder_png(&o).unwrap();
                    assert_eq!((image.largeur, image.hauteur), (640, 360));
                }
                ReponseArrivee::Rendu(Err(EchecDemande::Interne)) => perdus += 1,
                autre @ (ReponseArrivee::Rendu(Err(_))
                | ReponseArrivee::Image(_)
                | ReponseArrivee::Lu(_)
                | ReponseArrivee::Fait(_)) => panic!("{autre:?}"),
            }
        }
        assert!(perdus < 40);
    }
}
