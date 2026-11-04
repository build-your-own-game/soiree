// Un `Partage` n'est pas un `Message` (§ 2.2).
use jeu::prelude::*;

#[jeu::message]
pub struct Vue {
    pub image: Partage<Vec<u8>>,
}

fn main() {}
