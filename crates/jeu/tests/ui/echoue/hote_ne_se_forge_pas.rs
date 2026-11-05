// La crate de règles d'un jeu est sans `unsafe` (CONTRAT, « Pas de code `unsafe` ») :
// elle ne peut pas construire le jeton de l'hôte de la colle. Sans lui, une vue ne
// peut ni se rappeler sous le regard d'un joueur (`colle::appeler`), ni relire sa
// propre forme canonique, secrets compris (`colle::canonique`) : § 2.8, CAT-46.
#![forbid(unsafe_code)]

fn main() {
    let sans_bloc = jeu::colle::Hote::new();
    let avec_bloc = unsafe { jeu::colle::Hote::new() };
    let _ = (sans_bloc, avec_bloc);
}
