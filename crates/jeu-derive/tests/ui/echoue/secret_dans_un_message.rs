// Un `Secret` n'est pas un `Message` : une vue ne peut pas en contenir (§ 2.2, CAT-46).
use jeu::prelude::*;

#[jeu::message]
pub struct Vue {
    pub reponse: Secret<u8>,
}

fn main() {}
