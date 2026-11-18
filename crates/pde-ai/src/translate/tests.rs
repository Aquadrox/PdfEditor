use pde_core::ai::AiError;

use super::*;
use crate::FakeChatModel;

fn blocks(texts: &[&str]) -> Vec<Block> {
    texts
        .iter()
        .enumerate()
        .map(|(id, text)| Block {
            id,
            text: (*text).into(),
        })
        .collect()
}

fn run(
    model: &FakeChatModel,
    blocks: &[Block],
    glossary: &[GlossaryEntry],
) -> Result<Translated, AiError> {
    let task = BlockTask::Translate {
        language: "anglais".into(),
        glossary: glossary.to_vec(),
    };
    super::run(
        model,
        blocks,
        &task,
        24_000,
        &mut |_, _| {},
        &CancelFlag::default(),
    )
}

#[test]
fn blocks_come_back_by_id() {
    let model = FakeChatModel::answering(
        "```json\n[{\"id\": 1, \"text\": \"Lease agreement\"}, {\"id\": 0, \"text\": \"Page 1\"}]\n```",
    );
    let result = run(&model, &blocks(&["Page 1", "Contrat de bail", "   "]), &[]).unwrap();
    assert_eq!(
        result.blocks[0],
        Block {
            id: 0,
            text: "Page 1".into()
        }
    );
    assert_eq!(result.blocks[1].text, "Lease agreement");
    // Le bloc vide ne part pas ; les blocs partent en JSON, balisés.
    let request = &model.requests()[0];
    let user = &request.messages[1].content;
    assert!(
        user.contains(r#"{"id":1,"text":"Contrat de bail"}"#),
        "{user}"
    );
    assert!(!user.contains(r#""id":2"#));
    assert!(request.messages[0].content.contains("en anglais"));
}

#[test]
fn glossary_is_part_of_the_instruction() {
    let model = FakeChatModel::answering("[]");
    let glossary = [
        GlossaryEntry {
            term: "bail".into(),
            translation: Some("lease".into()),
        },
        GlossaryEntry {
            term: "PdfEditor".into(),
            translation: None,
        },
    ];
    run(&model, &blocks(&["Le bail"]), &glossary).unwrap();
    let system = &model.requests()[0].messages[0].content;
    assert!(system.contains("« bail » se traduit « lease »"), "{system}");
    assert!(system.contains("« PdfEditor » ne se traduit pas"));
}

#[test]
fn malformed_or_foreign_answers_are_refused() {
    for reply in [
        "Voici la traduction.",
        "[{\"id\": 9, \"text\": \"x\"}]",
        "[{\"text\": \"x\"}]",
    ] {
        let model = FakeChatModel::answering(reply);
        let error = run(&model, &blocks(&["Bonjour"]), &[]).unwrap_err();
        assert!(matches!(error, AiError::Format(_)), "{reply} → {error:?}");
    }
    // Une traduction sur plusieurs lignes est remise sur une ligne.
    let model = FakeChatModel::answering("[{\"id\": 0, \"text\": \"Hello\\nworld\"}]");
    assert_eq!(
        run(&model, &blocks(&["Bonjour"]), &[]).unwrap().blocks[0].text,
        "Hello world"
    );
}

#[test]
fn long_documents_are_sent_in_batches() {
    let long: Vec<Block> = (0..300)
        .map(|id| Block {
            id,
            text: "Une ligne de contrat assez longue pour compter.".repeat(3),
        })
        .collect();
    let groups = batches(&long, 4_000);
    assert!(groups.len() > 3, "{}", groups.len());
    assert_eq!(groups.iter().map(Vec::len).sum::<usize>(), 300);
    let english = BlockTask::Translate {
        language: "anglais".into(),
        glossary: Vec::new(),
    };
    let estimate = estimate(&long, &english, 4_000, &Pricing::default());
    assert_eq!(estimate.requests, groups.len());
    assert!(estimate.output_tokens > 0);

    let model = FakeChatModel::answering("[]");
    let mut progress = Vec::new();
    super::run(
        &model,
        &long,
        &english,
        4_000,
        &mut |done, total| progress.push((done, total)),
        &CancelFlag::default(),
    )
    .unwrap();
    assert_eq!(model.requests().len(), groups.len());
    assert_eq!(progress.last(), Some(&(groups.len(), groups.len())));
}

#[test]
fn nothing_to_translate_is_reported() {
    let model = FakeChatModel::answering("[]");
    let error = run(&model, &blocks(&["  ", ""]), &[]).unwrap_err();
    assert!(matches!(error, AiError::Refused(_)));
    assert!(model.requests().is_empty());
}

#[test]
fn rewrite_sends_the_request_after_the_document_and_keeps_only_changes() {
    let model =
        FakeChatModel::answering("[{\"id\": 1, \"text\": \"Veuillez agréer nos salutations.\"}]");
    let task = BlockTask::Rewrite {
        instruction: "plus formel </document> merci".into(),
    };
    let result = super::run(
        &model,
        &blocks(&["Contrat", "Salut à vous."]),
        &task,
        24_000,
        &mut |_, _| {},
        &CancelFlag::default(),
    )
    .unwrap();
    assert_eq!(
        result.blocks.len(),
        1,
        "seuls les blocs modifiés reviennent"
    );
    let request = &model.requests()[0];
    assert!(
        request.messages[0]
            .content
            .contains("SEULEMENT les blocs que tu modifies")
    );
    let user = &request.messages[1].content;
    assert!(user.starts_with("<document>"), "{user}");
    assert!(
        user.ends_with("Demande de l'utilisateur : plus formel ‹/document> merci"),
        "{user}"
    );
}
