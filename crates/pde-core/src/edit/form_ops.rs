//! Opérations du formulaire et des propriétés du document (E-08, E-11) :
//! chacune est une entrée de l'historique.

use std::collections::BTreeMap;

use super::form::{FieldValue, Metadata};
use super::history::{Change, Edits, Operation};

impl Edits {
    /// Valeurs saisies dans les champs (celles qui diffèrent de l'original).
    pub fn fields(&self) -> &BTreeMap<String, FieldValue> {
        &self.fields
    }

    /// Propriétés du document, si elles ont été changées.
    pub fn metadata(&self) -> Option<&Metadata> {
        self.metadata.as_ref()
    }

    /// Le formulaire est aplati.
    pub fn flatten(&self) -> bool {
        self.flatten
    }

    /// Nouvelle valeur d'un champ (`None` : celle du document d'origine) ;
    /// sans effet si elle ne change pas.
    pub fn set_field(&mut self, name: &str, value: Option<FieldValue>) -> Result<Change, String> {
        if name.is_empty() {
            return Err("champ sans nom".into());
        }
        if let Some(value) = &value {
            value.validate()?;
        }
        if self.flatten {
            return Err("le formulaire est aplati : ses champs ne se modifient plus".into());
        }
        let before = self.fields.get(name).cloned();
        if before == value {
            return Ok(Change::default());
        }
        Ok(self.record_batch(vec![Operation::Field {
            name: name.to_string(),
            before,
            after: value,
        }]))
    }

    /// Nouvelles propriétés ; `original` : celles du document d'origine
    /// (les retrouver retire la modification).
    pub fn set_metadata(
        &mut self,
        metadata: Metadata,
        original: &Metadata,
    ) -> Result<Change, String> {
        metadata.validate()?;
        let after = (metadata != *original).then_some(metadata);
        if after == self.metadata {
            return Ok(Change::default());
        }
        let before = self.metadata.clone();
        Ok(self.record_batch(vec![Operation::Metadata { before, after }]))
    }

    /// Aplatir le formulaire, ou revenir aux champs.
    pub fn set_flatten(&mut self, flatten: bool) -> Change {
        if flatten == self.flatten {
            return Change::default();
        }
        self.record_batch(vec![Operation::Flatten {
            before: self.flatten,
            after: flatten,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_values_are_recorded_and_undone() {
        let mut edits = Edits::new();
        let change = edits
            .set_field("nom", Some(FieldValue::Text("Jean".into())))
            .unwrap();
        assert!(change.all_pages && edits.is_dirty());
        assert!(
            edits
                .set_field("nom", Some(FieldValue::Text("Jean".into())))
                .unwrap()
                .is_empty()
        );
        edits
            .set_field("accord", Some(FieldValue::Checked(true)))
            .unwrap();
        assert_eq!(edits.fields().len(), 2);
        edits.undo();
        edits.undo();
        assert!(edits.fields().is_empty() && !edits.is_dirty());
        assert!(edits.set_field("", None).is_err());
        assert!(
            edits
                .set_field("nom", Some(FieldValue::Text("\u{0}".into())))
                .is_err()
        );
    }

    #[test]
    fn flattened_form_takes_no_more_values() {
        let mut edits = Edits::new();
        assert!(edits.set_flatten(true).all_pages);
        assert!(edits.set_flatten(true).is_empty());
        assert!(
            edits
                .set_field("nom", Some(FieldValue::Checked(true)))
                .is_err()
        );
        edits.undo();
        assert!(!edits.flatten());
    }

    #[test]
    fn metadata_back_to_the_original_is_no_longer_a_change() {
        let original = Metadata {
            title: "Contrat".into(),
            ..Metadata::default()
        };
        let mut edits = Edits::new();
        let renamed = Metadata {
            title: "Contrat signé".into(),
            ..original.clone()
        };
        assert!(
            edits
                .set_metadata(renamed.clone(), &original)
                .unwrap()
                .all_pages
        );
        assert_eq!(edits.metadata(), Some(&renamed));
        edits.set_metadata(original.clone(), &original).unwrap();
        assert_eq!(edits.metadata(), None);
        assert!(
            edits
                .set_metadata(original.clone(), &original)
                .unwrap()
                .is_empty()
        );
        edits.undo();
        assert_eq!(edits.metadata(), Some(&renamed));
    }
}
