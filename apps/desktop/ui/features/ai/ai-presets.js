// Préréglages des fournisseurs d'IA (repris de VoiceToText) : ils
// remplissent le formulaire des Réglages, tout reste modifiable.

const UNKNOWN_PRICE = { input_per_million: 0, cached_input_per_million: 0, output_per_million: 0 };
const NO_THINKING = '{"thinking":{"type":"disabled"}}';

export const PRESETS = [
  {
    id: "deepseek",
    label: "DeepSeek",
    base_url: "https://api.deepseek.com",
    model: "deepseek-flash",
    max_input_tokens: 60000,
    pricing: { input_per_million: 0.3, cached_input_per_million: 0.006, output_per_million: 1.2 },
    extra_body: NO_THINKING,
    note: "Tarif plein (heures de pointe) ; heures creuses à moitié prix.",
  },
  {
    id: "openai",
    label: "OpenAI",
    base_url: "https://api.openai.com/v1",
    model: "",
    max_input_tokens: 100000,
    pricing: UNKNOWN_PRICE,
    extra_body: "",
    note: "Indiquez le nom exact du modèle (voir platform.openai.com) et son tarif.",
  },
  {
    id: "mistral",
    label: "Mistral",
    base_url: "https://api.mistral.ai/v1",
    model: "mistral-small-latest",
    max_input_tokens: 60000,
    pricing: UNKNOWN_PRICE,
    extra_body: "",
    note: "Tarif à renseigner (voir mistral.ai).",
  },
  {
    id: "zai",
    label: "GLM (Z.ai)",
    base_url: "https://api.z.ai/api/paas/v4",
    model: "glm-5.2",
    max_input_tokens: 60000,
    pricing: UNKNOWN_PRICE,
    extra_body: NO_THINKING,
    note: "Tarif à renseigner.",
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1",
    model: "",
    max_input_tokens: 60000,
    pricing: UNKNOWN_PRICE,
    extra_body: "",
    note: "Modèle au format « fournisseur/modèle ». Tarif à renseigner.",
  },
  {
    id: "ollama",
    label: "Ollama (ce PC ou réseau local)",
    base_url: "http://localhost:11434/v1",
    model: "",
    max_input_tokens: 8000,
    pricing: UNKNOWN_PRICE,
    extra_body: "",
    note: "Rien ne quitte votre réseau, sans clé ni coût. « Tester ces réglages » liste les modèles installés.",
  },
  {
    id: "lmstudio",
    label: "LM Studio (ce PC)",
    base_url: "http://localhost:1234/v1",
    model: "",
    max_input_tokens: 8000,
    pricing: UNKNOWN_PRICE,
    extra_body: "",
    note: "Rien ne quitte ce PC, sans clé ni coût.",
  },
  { id: "custom", label: "Autre (compatible OpenAI)", note: "Adresse, modèle et tarif libres." },
];

/** Préréglage d'une adresse (« Autre » si aucun ne correspond). */
export function presetFor(baseUrl) {
  const url = (baseUrl ?? "").trim().replace(/\/+$/, "");
  return PRESETS.find((p) => p.base_url === url) ?? PRESETS.at(-1);
}

/** Fournisseur rempli par un préréglage, en gardant ce qu'il ne fixe pas. */
export function applyPreset(provider, preset) {
  if (!preset.base_url) return provider;
  return {
    ...provider,
    base_url: preset.base_url,
    model: preset.model,
    max_input_tokens: preset.max_input_tokens,
    extra_body: preset.extra_body,
    pricing: { ...preset.pricing, currency: provider.pricing?.currency || "USD" },
  };
}
