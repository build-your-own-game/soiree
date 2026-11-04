// Une raison dit quelque chose : trois mots au moins (§ 2.12, HIS-04, OUT-03).
const VIDE: jeu::Raison = jeu::raison!("");
const NA: jeu::Raison = jeu::raison!("n/a");

fn main() {
    let _ = (VIDE, NA);
}
