// `Rc` ne vit pas dans un état : mutabilité intérieure ou ordre variable (§ 2.7, § 8.2).
use std::rc::Rc;

#[jeu::etat]
pub struct Etat {
    pub champ: Rc<u8>,
}

fn main() {}
