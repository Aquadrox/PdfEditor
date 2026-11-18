//! IA vue de l'application : état et clé du fournisseur, analyses et
//! questions, assistant d'actions (IA-15) et ses recettes (IA-16).

use pde_ai::tasks::{PageText, Task};
use pde_core::ai::CancelFlag;
use pde_core::recipe::Recipe;
use pde_core::settings::Provider;
use pde_core::usage::{UsageSummary, day_of};

use super::{App, now};
use crate::actions::assistant::{self, ASSISTANT_ACTIONS, Proposal};
use crate::ai::{AiEvent, AiService, AiStatus, AnswerView, EstimateView, Scope, TestView};
use crate::error::{AppError, ErrorKind};
use crate::recipes::{RecipeStore, RecipeView};
use crate::viewer::PagesView;

impl App {
    /// Active l'IA.
    pub fn with_ai(mut self, ai: AiService) -> Self {
        self.ai = Some(ai);
        self
    }

    pub(super) fn ai_service(&self) -> Result<&AiService, AppError> {
        self.ai
            .as_ref()
            .ok_or_else(|| AppError::new(ErrorKind::Ai, "IA indisponible"))
    }

    pub fn ai_status(&self) -> Result<AiStatus, AppError> {
        Ok(self.ai_service()?.status(&self.settings.get().ai))
    }

    /// Clé API du fournisseur `provider` (vide : retirée).
    pub fn ai_set_key(&self, provider: &Provider, key: &str) -> Result<(), AppError> {
        self.ai_service()?.set_key(provider, key)
    }

    pub fn ai_test(&self, provider: &Provider) -> Result<TestView, AppError> {
        Ok(self.ai_service()?.test(provider))
    }

    pub fn ai_usage(&self) -> Result<UsageSummary, AppError> {
        Ok(self.ai_service()?.usage(&day_of(now())))
    }

    /// Texte de la portée, page par page.
    fn ai_pages(&self, id: u64, scope: &Scope) -> Result<Vec<PageText>, AppError> {
        let pages = match scope {
            Scope::Selection { text, page } => vec![(*page, text.clone())],
            Scope::Pages { pages } => self.viewer.plain_text(id, pages)?,
            Scope::Document => self.viewer.plain_text(id, &[])?,
        };
        Ok(pages
            .into_iter()
            .map(|(page, text)| PageText { page, text })
            .collect())
    }

    /// `recipe` : recette choisie pour l'assistant d'actions.
    pub fn ai_estimate(
        &self,
        id: u64,
        scope: &Scope,
        task: &Task,
        recipe: Option<&str>,
    ) -> Result<EstimateView, AppError> {
        let pages = self.ai_pages(id, scope)?;
        let (task, _) = self.with_recipe(task, recipe)?;
        Ok(self
            .ai_service()?
            .estimate(&self.settings.get().ai, &pages, &task, &day_of(now())))
    }

    /// Tâche et actions permises : la recette choisie précède la demande
    /// de l'utilisateur et limite les actions proposées.
    fn with_recipe(
        &self,
        task: &Task,
        recipe: Option<&str>,
    ) -> Result<(Task, Vec<String>), AppError> {
        let all = || ASSISTANT_ACTIONS.iter().map(|a| (*a).to_owned()).collect();
        let (Task::Assist(request), Some(id)) = (task, recipe) else {
            return Ok((task.clone(), all()));
        };
        let recipe = self.recipe_store()?.get(id)?.recipe;
        let mut text = format!(
            "Recette « {} » : {}",
            recipe.name.trim(),
            recipe.instruction.trim()
        );
        if !request.trim().is_empty() {
            text.push_str(&format!("\nPrécisions : {}", request.trim()));
        }
        Ok((Task::Assist(text), recipe.actions))
    }

    /// Nouvelle action IA : son numéro, pour l'annuler.
    pub fn ai_start(&self) -> Result<(u64, CancelFlag), AppError> {
        Ok(self.ai_service()?.start())
    }

    pub fn ai_cancel(&self, run: u64) {
        if let Some(ai) = &self.ai {
            ai.cancel(run);
        }
    }

    /// Exécute l'action `run` (bloquant ; `on_event` : avancement et réponse en flux).
    #[allow(clippy::too_many_arguments)]
    pub fn ai_run(
        &self,
        id: u64,
        scope: &Scope,
        task: &Task,
        recipe: Option<&str>,
        run: (u64, CancelFlag),
        consent: bool,
        on_event: &mut dyn FnMut(AiEvent),
    ) -> Result<AnswerView, AppError> {
        let pages = self.ai_pages(id, scope)?;
        let settings = self.settings.get().ai;
        let (task, allowed) = self.with_recipe(task, recipe)?;
        let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
        let mut answer = self.ai_service()?.run_with_tools(
            &settings,
            &pages,
            &task,
            Some(assistant::tools(&allowed)),
            &day_of(now()),
            run,
            consent,
            on_event,
        )?;
        // Assistant d'actions : chaque appel d'outil est vérifié sur le
        // document ; seules les propositions valables sont montrées.
        if matches!(task, Task::Assist(_)) {
            let scope = self.scope_pages(id, scope)?;
            for call in std::mem::take(&mut answer.tool_calls) {
                match assistant::parse(&call, &allowed)
                    .and_then(|p| self.viewer.preview_proposal(id, p, &scope))
                {
                    Ok(view) => answer.proposals.push(view),
                    Err(reason) => answer.rejected.push(reason),
                }
            }
        }
        Ok(answer)
    }

    /// Pages de la portée (rangs à partir de 0).
    pub(super) fn scope_pages(&self, id: u64, scope: &Scope) -> Result<Vec<usize>, AppError> {
        Ok(match scope {
            Scope::Selection { page, .. } => vec![*page],
            Scope::Pages { pages } => pages.clone(),
            Scope::Document => (0..self.viewer.document_view(id)?.pages.len()).collect(),
        })
    }

    /// Applique les propositions acceptées de l'assistant, en une opération.
    pub fn assistant_apply(
        &self,
        id: u64,
        scope: &Scope,
        proposals: &[Proposal],
    ) -> Result<PagesView, AppError> {
        let scope = self.scope_pages(id, scope)?;
        let author = self.settings.get().edit.author;
        self.viewer
            .apply_proposals(id, proposals, &scope, author.trim())
    }

    /// Recettes rangées dans `data_dir/recipes`.
    pub fn with_recipes(mut self, store: RecipeStore) -> Self {
        self.recipes = Some(store);
        self
    }

    fn recipe_store(&self) -> Result<&RecipeStore, AppError> {
        self.recipes
            .as_ref()
            .ok_or_else(|| AppError::new(ErrorKind::Ai, "recettes indisponibles"))
    }

    pub fn recipes(&self) -> Result<Vec<RecipeView>, AppError> {
        Ok(self.recipe_store()?.list())
    }

    pub fn recipe_save(&self, id: Option<&str>, recipe: Recipe) -> Result<RecipeView, AppError> {
        self.recipe_store()?.save(id, recipe)
    }

    pub fn recipe_delete(&self, id: &str) -> Result<(), AppError> {
        self.recipe_store()?.delete(id)
    }

    pub fn recipe_import(&self, path: &std::path::Path) -> Result<RecipeView, AppError> {
        self.recipe_store()?.import(path)
    }

    pub fn recipe_export(&self, id: &str, path: &std::path::Path) -> Result<(), AppError> {
        self.recipe_store()?.export(id, path)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use pde_ai::FakeTransport;
    use pde_core::secrets::SecretStore;

    use super::*;
    use crate::ai::UsageStore;
    use crate::app::AcceptedRewrite;
    use crate::fake::{FakeEngine, SIMPLE_PDF};
    use crate::recent::RecentFiles;
    use crate::settings_store::SettingsStore;
    use crate::viewer::translation::TranslationMode;

    /// Trousseau vide : le serveur local n'a pas besoin de clé.
    struct NoSecrets;

    impl SecretStore for NoSecrets {
        fn load(&self, _: &str) -> Option<String> {
            None
        }
        fn store(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn delete(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    /// Application avec un modèle local simulé et le document « Bonjour monde ».
    fn app(name: &str, replies: &[&str]) -> (App, Arc<FakeTransport>, u64, PathBuf) {
        let dir = std::env::temp_dir().join(format!("pde-app-ai-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[ai.provider]\nbase_url = \"http://localhost:11434/v1\"\nmodel = \"local\"\n",
        )
        .unwrap();
        let transport = Arc::new(FakeTransport::new(
            replies.iter().map(|r| Ok((200, (*r).to_owned()))).collect(),
        ));
        let app = App::new(
            Arc::new(FakeEngine::default()),
            SettingsStore::load(dir.join("config.toml")),
            RecentFiles::load(dir.join("recent.json")),
        )
        .with_ai(AiService::new(
            Arc::new(NoSecrets),
            transport.clone(),
            UsageStore::in_memory(),
        ))
        .with_recipes(RecipeStore::new(&dir));
        let pdf = dir.join("doc.pdf");
        std::fs::write(&pdf, SIMPLE_PDF).unwrap();
        let id = app.open_document(&pdf, None).unwrap().id;
        (app, transport, id, dir)
    }

    #[test]
    fn a_recipe_limits_the_actions_offered_and_accepted() {
        let calls = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[\
            {\"index\":0,\"function\":{\"name\":\"redact\",\"arguments\":\"{\\\"text\\\":\\\"monde\\\"}\"}},\
            {\"index\":1,\"function\":{\"name\":\"highlight\",\"arguments\":\"{\\\"text\\\":\\\"Bonjour\\\"}\"}}]}}]}\n\n\
            data: [DONE]\n";
        let (app, transport, id, dir) = app("recipe", &[calls]);
        let task = Task::Assist("sauf la signature".into());
        let start = app.ai_start().unwrap();
        let answer = app
            .ai_run(
                id,
                &Scope::Document,
                &task,
                Some("integree-rgpd"),
                start,
                true,
                &mut |_| {},
            )
            .unwrap();
        assert_eq!(answer.proposals.len(), 1);
        assert_eq!(answer.proposals[0].summary, "Caviarder « monde »");
        assert_eq!(answer.rejected, ["action « highlight » non permise"]);

        let sent = transport.sent.lock().unwrap();
        let body: serde_json::Value = serde_json::from_str(&sent[0].2).unwrap();
        assert_eq!(
            body["tools"].as_array().unwrap().len(),
            1,
            "seul l'outil de la recette"
        );
        let user = body["messages"][1]["content"].as_str().unwrap();
        assert!(
            user.contains("Recette « Anonymiser selon le RGPD »"),
            "{user}"
        );
        assert!(user.ends_with("Précisions : sauf la signature"), "{user}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn translation_is_placed_on_the_document_or_kept_as_text() {
        let reply =
            r#"{"choices":[{"message":{"content":"[{\"id\":0,\"text\":\"Hello PdfEditor\"}]"}}]}"#;
        let (app, transport, id, dir) = app("translate", &[reply, reply]);
        let scope = Scope::Pages { pages: vec![0] };
        let estimate = app.ai_translate_estimate(id, &scope, "anglais").unwrap();
        assert_eq!(estimate.requests, 1);

        let start = app.ai_start().unwrap();
        let text = app
            .ai_translate(
                id,
                &scope,
                "anglais",
                TranslationMode::Text,
                start,
                true,
                &mut |_| {},
            )
            .unwrap();
        assert_eq!(text.text, "## Page 1\nHello PdfEditor\n");
        assert!(text.applied.is_none() && text.missing == 0);
        assert!(app.viewer.edit_view(id).unwrap().elements.is_empty());

        let start = app.ai_start().unwrap();
        let placed = app
            .ai_translate(
                id,
                &scope,
                "anglais",
                TranslationMode::Replace,
                start,
                true,
                &mut |_| {},
            )
            .unwrap();
        assert_eq!(placed.applied.map(|a| a.blocks), Some(1));
        assert_eq!(app.viewer.edit_view(id).unwrap().elements.len(), 1);
        let sent = transport.sent.lock().unwrap();
        assert!(
            sent[0].2.contains("Bonjour PdfEditor"),
            "les lignes partent"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rewrite_proposes_changed_lines_and_applies_only_accepted_ones() {
        let reply = r#"{"choices":[{"message":{"content":"[{\"id\":0,\"text\":\"Bonjour cher PdfEditor\"}]"}}]}"#;
        let (app, transport, id, dir) = app("rewrite", &[reply]);
        let scope = Scope::Pages { pages: vec![0] };
        assert!(
            app.ai_rewrite_estimate(id, &scope, "  ").is_err(),
            "consigne vide"
        );
        let start = app.ai_start().unwrap();
        let result = app
            .ai_rewrite(id, &scope, "plus chaleureux", start, true, &mut |_| {})
            .unwrap();
        assert_eq!(result.proposals.len(), 1);
        let proposal = &result.proposals[0];
        assert_eq!(
            (proposal.before.as_str(), proposal.after.as_str()),
            ("Bonjour PdfEditor", "Bonjour cher PdfEditor")
        );
        // Rien n'est posé avant l'accord.
        assert!(app.viewer.edit_view(id).unwrap().elements.is_empty());
        let body = &transport.sent.lock().unwrap()[0].2;
        assert!(
            body.contains("Demande de l'utilisateur : plus chaleureux"),
            "{body}"
        );

        let accepted = AcceptedRewrite {
            page: proposal.page,
            objects: proposal.objects.clone(),
            before: proposal.before.clone(),
            text: proposal.after.clone(),
        };
        // Ligne qui a changé depuis : refusée.
        let stale = AcceptedRewrite {
            before: "autre texte".into(),
            ..accepted.clone()
        };
        assert!(app.ai_rewrite_apply(id, &[stale]).is_err());
        let placed = app.ai_rewrite_apply(id, &[accepted]).unwrap();
        assert_eq!(placed.blocks, 1);
        assert_eq!(app.viewer.edit_view(id).unwrap().elements.len(), 1);
        app.viewer.undo(id).unwrap();
        assert!(app.viewer.edit_view(id).unwrap().elements.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_recipe_is_refused_before_sending() {
        let (app, transport, id, dir) = app("unknown", &[]);
        let task = Task::Assist(String::new());
        let start = app.ai_start().unwrap();
        let error = app
            .ai_run(
                id,
                &Scope::Document,
                &task,
                Some("disparue"),
                start,
                true,
                &mut |_| {},
            )
            .unwrap_err();
        assert!(error.message.contains("recette inconnue"));
        assert!(transport.sent.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
