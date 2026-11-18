//! Travail par blocs vu de l'application : traduction (IA-06 à IA-08)
//! posée sur le document ou gardée en texte, et modification par consigne
//! (IA-09) proposée bloc par bloc, appliquée sur accord.

use pde_ai::translate::{Block, BlockTask};
use pde_core::ai::CancelFlag;
use pde_core::usage::day_of;
use serde::{Deserialize, Serialize};

use super::{App, now};
use crate::ai::{AiEvent, EstimateView, Scope};
use crate::error::{AppError, ErrorKind};
use crate::text_view::BoxView;
use crate::viewer::translation::{SourceBlock, TranslatedView, TranslationMode};

/// Traduction terminée, pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TranslationResult {
    /// Texte traduit, page par page (Markdown simple).
    pub text: String,
    pub cost: f64,
    pub currency: String,
    pub model: String,
    pub fallback_used: bool,
    /// Traduction posée sur le document (sauf en texte seul).
    pub applied: Option<TranslatedView>,
    /// Lignes que le modèle n'a pas rendues (laissées telles quelles).
    pub missing: usize,
}

/// Traduction vers `language`, avec le glossaire des réglages.
fn translate_task(settings: &pde_core::settings::Ai, language: &str) -> BlockTask {
    BlockTask::Translate {
        language: language.to_owned(),
        glossary: settings.translation.glossary_entries(),
    }
}

/// « ## Page n » puis les lignes traduites de chaque page.
fn translation_text(translated: &[(SourceBlock, String)]) -> String {
    let mut text = String::new();
    let mut page = None;
    for (source, line) in translated {
        if page != Some(source.page) {
            if page.is_some() {
                text.push('\n');
            }
            text.push_str(&format!("## Page {}\n", source.page + 1));
            page = Some(source.page);
        }
        text.push_str(line.trim());
        text.push('\n');
    }
    text
}

/// Modification proposée pour une ligne (IA-09).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RewriteProposal {
    pub page: usize,
    /// Rangs des objets texte de la ligne : la désignent à l'application.
    pub objects: Vec<u32>,
    pub bounds: BoxView,
    pub before: String,
    pub after: String,
}

/// Propositions de modification, et ce qu'elles ont coûté.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RewriteResult {
    pub proposals: Vec<RewriteProposal>,
    pub cost: f64,
    pub currency: String,
    pub model: String,
    pub fallback_used: bool,
}

/// Modification acceptée par l'utilisateur, revérifiée avant d'être posée.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AcceptedRewrite {
    pub page: usize,
    pub objects: Vec<u32>,
    /// Texte de la ligne quand la modification a été proposée.
    pub before: String,
    pub text: String,
}

impl App {
    /// Blocs de la portée à traduire ; une sélection garde les lignes
    /// qu'elle contient.
    pub(super) fn translation_sources(
        &self,
        id: u64,
        scope: &Scope,
    ) -> Result<Vec<SourceBlock>, AppError> {
        let pages = self.scope_pages(id, scope)?;
        let mut blocks = self.viewer.source_blocks(id, &pages)?;
        if let Scope::Selection { text, .. } = scope {
            let selected = text.split_whitespace().collect::<Vec<_>>().join(" ");
            blocks.retain(|b| {
                let line = b
                    .block
                    .text
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                !line.is_empty() && selected.contains(&line)
            });
        }
        Ok(blocks)
    }

    fn as_blocks(sources: &[SourceBlock]) -> Vec<Block> {
        sources
            .iter()
            .enumerate()
            .map(|(id, s)| Block {
                id,
                text: s.block.text.clone(),
            })
            .collect()
    }

    pub fn ai_translate_estimate(
        &self,
        id: u64,
        scope: &Scope,
        language: &str,
    ) -> Result<EstimateView, AppError> {
        let blocks = Self::as_blocks(&self.translation_sources(id, scope)?);
        let settings = self.settings.get().ai;
        Ok(self.ai_service()?.estimate_translation(
            &settings,
            &blocks,
            &translate_task(&settings, language),
            &day_of(now()),
        ))
    }

    /// Traduit la portée (bloquant) puis, sauf en texte seul, pose la
    /// traduction sur le document en une opération.
    #[allow(clippy::too_many_arguments)]
    pub fn ai_translate(
        &self,
        id: u64,
        scope: &Scope,
        language: &str,
        mode: TranslationMode,
        run: (u64, CancelFlag),
        consent: bool,
        on_event: &mut dyn FnMut(AiEvent),
    ) -> Result<TranslationResult, AppError> {
        let sources = self.translation_sources(id, scope)?;
        let settings = self.settings.get().ai;
        let answer = self.ai_service()?.translate(
            &settings,
            &Self::as_blocks(&sources),
            &translate_task(&settings, language),
            &day_of(now()),
            run,
            consent,
            on_event,
        )?;
        let translated: Vec<(SourceBlock, String)> = answer
            .blocks
            .iter()
            .filter_map(|b| Some((sources.get(b.id)?.clone(), b.text.clone())))
            .collect();
        let missing = sources
            .iter()
            .filter(|s| !s.block.text.trim().is_empty())
            .count()
            .saturating_sub(translated.len());
        let text = translation_text(&translated);
        let applied = match mode {
            TranslationMode::Text => None,
            _ if translated.is_empty() => None,
            _ => Some(self.viewer.apply_translation(
                id,
                mode,
                &translated,
                settings.translation.min_size,
            )?),
        };
        Ok(TranslationResult {
            text,
            cost: answer.cost,
            currency: answer.currency,
            model: answer.model,
            fallback_used: answer.fallback_used,
            applied,
            missing,
        })
    }

    pub fn ai_rewrite_estimate(
        &self,
        id: u64,
        scope: &Scope,
        instruction: &str,
    ) -> Result<EstimateView, AppError> {
        let blocks = Self::as_blocks(&self.translation_sources(id, scope)?);
        let task = rewrite_task(instruction)?;
        Ok(self.ai_service()?.estimate_translation(
            &self.settings.get().ai,
            &blocks,
            &task,
            &day_of(now()),
        ))
    }

    /// Demande au modèle les lignes à modifier (bloquant) ; rien n'est posé.
    #[allow(clippy::too_many_arguments)]
    pub fn ai_rewrite(
        &self,
        id: u64,
        scope: &Scope,
        instruction: &str,
        run: (u64, CancelFlag),
        consent: bool,
        on_event: &mut dyn FnMut(AiEvent),
    ) -> Result<RewriteResult, AppError> {
        let task = rewrite_task(instruction)?;
        let sources = self.translation_sources(id, scope)?;
        let answer = self.ai_service()?.translate(
            &self.settings.get().ai,
            &Self::as_blocks(&sources),
            &task,
            &day_of(now()),
            run,
            consent,
            on_event,
        )?;
        let proposals = answer
            .blocks
            .iter()
            .filter_map(|b| {
                let source = sources.get(b.id)?;
                let after = b.text.trim();
                (after != source.block.text.trim()).then(|| RewriteProposal {
                    page: source.page,
                    objects: source.block.objects.clone(),
                    bounds: source.block.bounds.clone(),
                    before: source.block.text.clone(),
                    after: after.to_owned(),
                })
            })
            .collect();
        Ok(RewriteResult {
            proposals,
            cost: answer.cost,
            currency: answer.currency,
            model: answer.model,
            fallback_used: answer.fallback_used,
        })
    }

    /// Pose les modifications acceptées en une opération, si chaque ligne
    /// est toujours celle qui a été proposée.
    pub fn ai_rewrite_apply(
        &self,
        id: u64,
        accepted: &[AcceptedRewrite],
    ) -> Result<TranslatedView, AppError> {
        let mut pages: Vec<usize> = accepted.iter().map(|a| a.page).collect();
        pages.sort_unstable();
        pages.dedup();
        let sources = self.viewer.source_blocks(id, &pages)?;
        let mut placed = Vec::with_capacity(accepted.len());
        for item in accepted {
            let source = sources
                .iter()
                .find(|s| s.page == item.page && s.block.objects == item.objects)
                .filter(|s| s.block.text == item.before)
                .ok_or_else(|| {
                    AppError::new(
                        ErrorKind::InvalidRequest,
                        "le document a changé depuis la proposition : demandez-la à nouveau",
                    )
                })?;
            placed.push((source.clone(), item.text.clone()));
        }
        let min_size = self.settings.get().ai.translation.min_size;
        self.viewer
            .apply_translation(id, TranslationMode::Replace, &placed, min_size)
    }
}

/// Modification selon `instruction`, qui ne peut pas être vide.
fn rewrite_task(instruction: &str) -> Result<BlockTask, AppError> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err(AppError::new(
            ErrorKind::InvalidRequest,
            "consigne de modification vide",
        ));
    }
    Ok(BlockTask::Rewrite {
        instruction: instruction.to_owned(),
    })
}
