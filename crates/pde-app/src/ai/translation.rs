//! Traduction vue du service IA (IA-06 à IA-08) : estimation, accord et
//! plafond comme pour les autres actions, glossaire des réglages, profil de
//! secours. Seules des longueurs et des durées vont au journal.

use pde_ai::translate::{self, Block};
use pde_core::ai::{AiError, CancelFlag};
use pde_core::settings::{Ai, Provider};

use super::{AiEvent, AiService, EstimateView, ai_error};
use crate::error::AppError;
use crate::lock;

/// Blocs traduits et ce qu'ils ont coûté.
#[derive(Debug, Clone, PartialEq)]
pub struct TranslationAnswer {
    pub blocks: Vec<Block>,
    pub cost: f64,
    pub currency: String,
    pub model: String,
    pub fallback_used: bool,
}

impl AiService {
    pub fn estimate_translation(
        &self,
        ai: &Ai,
        blocks: &[Block],
        language: &str,
        today: &str,
    ) -> EstimateView {
        let provider = &ai.provider;
        let e = translate::estimate(
            blocks,
            language,
            &ai.translation.glossary_entries(),
            provider.max_input_tokens as usize,
            &provider.pricing,
        );
        self.estimate_view(
            ai,
            (e.requests, e.input_tokens, e.output_tokens),
            e.cost,
            e.characters,
            today,
        )
    }

    /// Traduit les blocs (bloquant) ; `on_event` reçoit l'avancement.
    #[allow(clippy::too_many_arguments)]
    pub fn translate(
        &self,
        ai: &Ai,
        blocks: &[Block],
        language: &str,
        today: &str,
        run: (u64, CancelFlag),
        consent: bool,
        on_event: &mut dyn FnMut(AiEvent),
    ) -> Result<TranslationAnswer, AppError> {
        let (id, cancel) = run;
        let result = self.translate_inner(ai, blocks, language, today, &cancel, consent, on_event);
        lock(&self.runs).remove(&id);
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn translate_inner(
        &self,
        ai: &Ai,
        blocks: &[Block],
        language: &str,
        today: &str,
        cancel: &CancelFlag,
        consent: bool,
        on_event: &mut dyn FnMut(AiEvent),
    ) -> Result<TranslationAnswer, AppError> {
        let estimate = self.estimate_translation(ai, blocks, language, today);
        self.check_send(ai, &estimate, consent)?;
        let started = std::time::Instant::now();
        let glossary = ai.translation.glossary_entries();
        let attempt = |provider: &Provider, on_event: &mut dyn FnMut(AiEvent)| {
            let model = self.model(provider);
            let translated = translate::translate(
                model.as_ref(),
                blocks,
                language,
                &glossary,
                provider.max_input_tokens as usize,
                &mut |done, total| on_event(AiEvent::Reading { done, total }),
                cancel,
            )?;
            Ok::<_, AiError>((translated, model.label(), provider.pricing.clone()))
        };
        let (result, fallback_used) = match attempt(&ai.provider, on_event) {
            Err(error) if !matches!(error, AiError::Cancelled | AiError::Refused(_)) => {
                match ai.fallback_provider() {
                    Some(fallback) => {
                        log::warn!("fournisseur d'IA en échec ({error}) : profil de secours");
                        (attempt(fallback, on_event), true)
                    }
                    None => (Err(error), false),
                }
            }
            other => (other, false),
        };
        let (translated, model, pricing) = result.map_err(ai_error)?;
        self.usage.record(today, &translated.usage, &pricing);
        log::info!(
            "traduction terminée : {} blocs, {} caractères envoyés, {} ms",
            translated.blocks.len(),
            estimate.characters,
            started.elapsed().as_millis()
        );
        Ok(TranslationAnswer {
            blocks: translated.blocks,
            cost: pricing.cost(&translated.usage),
            currency: pricing.currency,
            model,
            fallback_used,
        })
    }
}
