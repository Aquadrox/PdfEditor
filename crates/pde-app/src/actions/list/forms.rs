//! Actions du mode Formulaire (E-12) : créer des champs, les détecter,
//! essayer le formulaire.

use super::super::{ActionInfo, Mode};
use super::action;

const FORM: &str = "Formulaire";

pub(super) fn form() -> Vec<ActionInfo> {
    vec![
        action("mode_form", FORM, "Mode Formulaire", "Créer ou modifier les champs d'un formulaire : texte, date, montant, cases, listes, signatures.", "#mode-form")
            .keywords(&["formulaire", "champ", "créer", "concevoir", "modifier"]),
        action("add_text_field", FORM, "Champ texte", "Ajouter un champ à remplir : choisissez l'outil Texte puis tracez son cadre sur la page.", "[data-field-tool=\"text\"]")
            .mode(Mode::Form)
            .keywords(&["champ", "zone", "saisie", "remplir", "texte"]),
        action("add_date_field", FORM, "Champ date", "Ajouter un champ date vérifié, écrit au format choisi (jj/mm/aaaa), avec un calendrier.", "[data-field-tool=\"date\"]")
            .mode(Mode::Form)
            .keywords(&["date", "calendrier", "jour", "naissance"]),
        action("add_amount_field", FORM, "Champ montant", "Ajouter un champ montant (euros, deux décimales) ; un total peut en faire la somme.", "[data-field-tool=\"amount\"]")
            .mode(Mode::Form)
            .keywords(&["montant", "prix", "euros", "total", "somme", "nombre"]),
        action("add_checkbox_field", FORM, "Case à cocher", "Ajouter une case à cocher au formulaire.", "[data-field-tool=\"checkbox\"]")
            .mode(Mode::Form)
            .keywords(&["case", "cocher", "oui", "non"]),
        action("add_choice_field", FORM, "Liste déroulante", "Ajouter une liste d'options à choisir.", "[data-field-tool=\"dropdown\"]")
            .mode(Mode::Form)
            .keywords(&["liste", "menu", "options", "choix"]),
        action("add_signature_field", FORM, "Emplacement de signature", "Placer un champ de signature vide, à signer ensuite dans le mode Signer.", "[data-field-tool=\"signature\"]")
            .mode(Mode::Form)
            .keywords(&["signature", "signer", "signataire", "visa"]),
        action("detect_form_fields", FORM, "Détecter les champs", "Proposer des champs là où la page attend une écriture (traits, pointillés, cases, « Libellé : »), à accepter.", "#form-detect")
            .mode(Mode::Form)
            .keywords(&["détecter", "automatique", "reconnaître", "scanner", "formulaire papier"]),
        action("test_form", FORM, "Tester le formulaire", "Passer en mode Lire pour remplir le formulaire et l'essayer.", "#form-test")
            .mode(Mode::Form)
            .keywords(&["essayer", "remplir", "aperçu"]),
    ]
}
