// Un regard est construit par le commun seul (§ 2.8) : une vue qui forgerait le
// regard de chaque joueur ouvrirait ses secrets sur la télé.
use jeu::prelude::*;

fn main() {
    let _: Regard = Regard::du_fil(jeu_abi::RegardFil::Appareil(42));
}
