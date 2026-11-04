// `HashMap` ne vit pas dans un état : mutabilité intérieure ou ordre variable (§ 2.7, § 8.2).
use std::collections::HashMap;

#[jeu::etat]
pub struct Etat {
    pub champ: HashMap<u8, u8>,
}

fn main() {}
