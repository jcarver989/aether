// Auto-generated from models.dev — do not edit manually
// Run `cargo run --bin llm-catalog-codegen` to regenerate

use std::borrow::Cow;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnthropicModel {
    Claude35Haiku20241022,
    Claude35Sonnet20240620,
    Claude35Sonnet20241022,
    Claude37Sonnet20250219,
    Claude3Haiku20240307,
    Claude3Opus20240229,
    Claude3Sonnet20240229,
    ClaudeHaiku45,
    ClaudeHaiku4520251001,
    ClaudeOpus40,
    ClaudeOpus41,
    ClaudeOpus4120250805,
    ClaudeOpus420250514,
    ClaudeOpus45,
    ClaudeOpus4520251101,
    ClaudeOpus46,
    ClaudeSonnet40,
    ClaudeSonnet420250514,
    ClaudeSonnet45,
    ClaudeSonnet4520250929,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeepSeekModel {
    DeepseekChat,
    DeepseekReasoner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeminiModel {
    Gemini15Flash,
    Gemini15Flash8b,
    Gemini15Pro,
    Gemini20Flash,
    Gemini20FlashLite,
    Gemini25Flash,
    Gemini25FlashLite,
    Gemini25FlashLitePreview0617,
    Gemini25FlashLitePreview092025,
    Gemini25FlashPreview0417,
    Gemini25FlashPreview0520,
    Gemini25FlashPreview092025,
    Gemini25Pro,
    Gemini25ProPreview0506,
    Gemini25ProPreview0605,
    Gemini3FlashPreview,
    Gemini3ProPreview,
    GeminiLive25Flash,
    GeminiLive25FlashPreviewNativeAudio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoonshotModel {
    KimiK20711Preview,
    KimiK20905Preview,
    KimiK2Thinking,
    KimiK2ThinkingTurbo,
    KimiK2TurboPreview,
    KimiK25,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenRouterModel {
    AnthropicClaude35Haiku,
    AnthropicClaude37Sonnet,
    AnthropicClaudeHaiku45,
    AnthropicClaudeOpus4,
    AnthropicClaudeOpus41,
    AnthropicClaudeOpus45,
    AnthropicClaudeOpus46,
    AnthropicClaudeSonnet4,
    AnthropicClaudeSonnet45,
    ArceeAiTrinityLargePreviewFree,
    ArceeAiTrinityMiniFree,
    CognitivecomputationsDolphin30Mistral24b,
    CognitivecomputationsDolphin30R1Mistral24b,
    DeepseekDeepseekChatV31,
    DeepseekDeepseekR10528Qwen38bFree,
    DeepseekDeepseekR1Free,
    DeepseekDeepseekV31Terminus,
    DeepseekDeepseekV31TerminusExacto,
    DeepseekDeepseekV32,
    DeepseekDeepseekV32Speciale,
    GoogleGemini20Flash001,
    GoogleGemini20FlashExpFree,
    GoogleGemini25Flash,
    GoogleGemini25FlashLite,
    GoogleGemini25FlashLitePreview092025,
    GoogleGemini25FlashPreview092025,
    GoogleGemini25Pro,
    GoogleGemini25ProPreview0506,
    GoogleGemini25ProPreview0605,
    GoogleGemini3FlashPreview,
    GoogleGemini3ProPreview,
    GoogleGemma327bIt,
    GoogleGemma327bItFree,
    KwaipilotKatCoderProFree,
    MetaLlamaLlama3370bInstructFree,
    MetaLlamaLlama4ScoutFree,
    MicrosoftMaiDsR1Free,
    MinimaxMinimax01,
    MinimaxMinimaxM1,
    MinimaxMinimaxM2,
    MinimaxMinimaxM21,
    MinimaxMinimaxM25,
    MistralaiCodestral2508,
    MistralaiDevstral2512,
    MistralaiDevstral2512Free,
    MistralaiDevstralMedium2507,
    MistralaiDevstralSmall2505,
    MistralaiDevstralSmall2505Free,
    MistralaiDevstralSmall2507,
    MistralaiMistral7bInstructFree,
    MistralaiMistralMedium3,
    MistralaiMistralMedium31,
    MistralaiMistralNemoFree,
    MistralaiMistralSmall3124bInstruct,
    MistralaiMistralSmall3224bInstruct,
    MistralaiMistralSmall3224bInstructFree,
    MoonshotaiKimiDev72bFree,
    MoonshotaiKimiK2,
    MoonshotaiKimiK20905,
    MoonshotaiKimiK20905Exacto,
    MoonshotaiKimiK2Thinking,
    MoonshotaiKimiK25,
    MoonshotaiKimiK2Free,
    NousresearchDeephermes3Llama38bPreview,
    NousresearchHermes4405b,
    NousresearchHermes470b,
    NvidiaNemotron3Nano30bA3bFree,
    NvidiaNemotronNano12bV2VlFree,
    NvidiaNemotronNano9bV2,
    NvidiaNemotronNano9bV2Free,
    OpenaiGpt41,
    OpenaiGpt41Mini,
    OpenaiGpt4oMini,
    OpenaiGpt5,
    OpenaiGpt5Codex,
    OpenaiGpt5Image,
    OpenaiGpt5Mini,
    OpenaiGpt5Nano,
    OpenaiGpt5Pro,
    OpenaiGpt51,
    OpenaiGpt51Chat,
    OpenaiGpt51Codex,
    OpenaiGpt51CodexMax,
    OpenaiGpt51CodexMini,
    OpenaiGpt52,
    OpenaiGpt52Chat,
    OpenaiGpt52Codex,
    OpenaiGpt52Pro,
    OpenaiGptOss120b,
    OpenaiGptOss120bExacto,
    OpenaiGptOss120bFree,
    OpenaiGptOss20b,
    OpenaiGptOss20bFree,
    OpenaiGptOssSafeguard20b,
    OpenaiO4Mini,
    OpenrouterAuroraAlpha,
    OpenrouterSherlockDashAlpha,
    OpenrouterSherlockThinkAlpha,
    QwenQwen25Vl7bInstructFree,
    QwenQwen25Vl32bInstructFree,
    QwenQwen25Vl72bInstructFree,
    QwenQwen314bFree,
    QwenQwen3235bA22b0725,
    QwenQwen3235bA22b0725Free,
    QwenQwen3235bA22bThinking2507,
    QwenQwen3235bA22bFree,
    QwenQwen330bA3bInstruct2507,
    QwenQwen330bA3bThinking2507,
    QwenQwen330bA3bFree,
    QwenQwen332bFree,
    QwenQwen34bFree,
    QwenQwen38bFree,
    QwenQwen3Coder,
    QwenQwen3Coder30bA3bInstruct,
    QwenQwen3CoderFlash,
    QwenQwen3CoderExacto,
    QwenQwen3CoderFree,
    QwenQwen3Max,
    QwenQwen3Next80bA3bInstruct,
    QwenQwen3Next80bA3bInstructFree,
    QwenQwen3Next80bA3bThinking,
    QwenQwq32bFree,
    RekaaiRekaFlash3,
    SarvamaiSarvamMFree,
    StepfunStep35Flash,
    StepfunStep35FlashFree,
    ThudmGlmZ132bFree,
    TngtechTngR1tChimeraFree,
    XAiGrok3,
    XAiGrok3Beta,
    XAiGrok3Mini,
    XAiGrok3MiniBeta,
    XAiGrok4,
    XAiGrok4Fast,
    XAiGrok41Fast,
    XAiGrokCodeFast1,
    XiaomiMimoV2Flash,
    ZAiGlm45,
    ZAiGlm45Air,
    ZAiGlm45v,
    ZAiGlm46,
    ZAiGlm46Exacto,
    ZAiGlm47,
    ZAiGlm47Flash,
    ZAiGlm5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZAiModel {
    Glm45,
    Glm45Air,
    Glm45Flash,
    Glm45v,
    Glm46,
    Glm46v,
    Glm47,
    Glm47Flash,
    Glm5,
}

/// A model from a specific provider
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LlmModel {
    Anthropic(AnthropicModel),
    DeepSeek(DeepSeekModel),
    Gemini(GeminiModel),
    Moonshot(MoonshotModel),
    OpenRouter(OpenRouterModel),
    ZAi(ZAiModel),
    Ollama(String),
    LlamaCpp(String),
}

impl From<AnthropicModel> for LlmModel {
    fn from(m: AnthropicModel) -> Self {
        LlmModel::Anthropic(m)
    }
}

impl From<DeepSeekModel> for LlmModel {
    fn from(m: DeepSeekModel) -> Self {
        LlmModel::DeepSeek(m)
    }
}

impl From<GeminiModel> for LlmModel {
    fn from(m: GeminiModel) -> Self {
        LlmModel::Gemini(m)
    }
}

impl From<MoonshotModel> for LlmModel {
    fn from(m: MoonshotModel) -> Self {
        LlmModel::Moonshot(m)
    }
}

impl From<OpenRouterModel> for LlmModel {
    fn from(m: OpenRouterModel) -> Self {
        LlmModel::OpenRouter(m)
    }
}

impl From<ZAiModel> for LlmModel {
    fn from(m: ZAiModel) -> Self {
        LlmModel::ZAi(m)
    }
}

impl LlmModel {
    /// Raw model ID (e.g. "claude-opus-4-6", "llama3.2")
    pub fn model_id(&self) -> Cow<'static, str> {
        match self {
            LlmModel::Anthropic(AnthropicModel::Claude35Haiku20241022) => {
                Cow::Borrowed("claude-3-5-haiku-20241022")
            }
            LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20240620) => {
                Cow::Borrowed("claude-3-5-sonnet-20240620")
            }
            LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20241022) => {
                Cow::Borrowed("claude-3-5-sonnet-20241022")
            }
            LlmModel::Anthropic(AnthropicModel::Claude37Sonnet20250219) => {
                Cow::Borrowed("claude-3-7-sonnet-20250219")
            }
            LlmModel::Anthropic(AnthropicModel::Claude3Haiku20240307) => {
                Cow::Borrowed("claude-3-haiku-20240307")
            }
            LlmModel::Anthropic(AnthropicModel::Claude3Opus20240229) => {
                Cow::Borrowed("claude-3-opus-20240229")
            }
            LlmModel::Anthropic(AnthropicModel::Claude3Sonnet20240229) => {
                Cow::Borrowed("claude-3-sonnet-20240229")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeHaiku45) => Cow::Borrowed("claude-haiku-4-5"),
            LlmModel::Anthropic(AnthropicModel::ClaudeHaiku4520251001) => {
                Cow::Borrowed("claude-haiku-4-5-20251001")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus40) => Cow::Borrowed("claude-opus-4-0"),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus41) => Cow::Borrowed("claude-opus-4-1"),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus4120250805) => {
                Cow::Borrowed("claude-opus-4-1-20250805")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus420250514) => {
                Cow::Borrowed("claude-opus-4-20250514")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus45) => Cow::Borrowed("claude-opus-4-5"),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus4520251101) => {
                Cow::Borrowed("claude-opus-4-5-20251101")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus46) => Cow::Borrowed("claude-opus-4-6"),
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet40) => {
                Cow::Borrowed("claude-sonnet-4-0")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet420250514) => {
                Cow::Borrowed("claude-sonnet-4-20250514")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet45) => {
                Cow::Borrowed("claude-sonnet-4-5")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet4520250929) => {
                Cow::Borrowed("claude-sonnet-4-5-20250929")
            }
            LlmModel::DeepSeek(DeepSeekModel::DeepseekChat) => Cow::Borrowed("deepseek-chat"),
            LlmModel::DeepSeek(DeepSeekModel::DeepseekReasoner) => {
                Cow::Borrowed("deepseek-reasoner")
            }
            LlmModel::Gemini(GeminiModel::Gemini15Flash) => Cow::Borrowed("gemini-1.5-flash"),
            LlmModel::Gemini(GeminiModel::Gemini15Flash8b) => Cow::Borrowed("gemini-1.5-flash-8b"),
            LlmModel::Gemini(GeminiModel::Gemini15Pro) => Cow::Borrowed("gemini-1.5-pro"),
            LlmModel::Gemini(GeminiModel::Gemini20Flash) => Cow::Borrowed("gemini-2.0-flash"),
            LlmModel::Gemini(GeminiModel::Gemini20FlashLite) => {
                Cow::Borrowed("gemini-2.0-flash-lite")
            }
            LlmModel::Gemini(GeminiModel::Gemini25Flash) => Cow::Borrowed("gemini-2.5-flash"),
            LlmModel::Gemini(GeminiModel::Gemini25FlashLite) => {
                Cow::Borrowed("gemini-2.5-flash-lite")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview0617) => {
                Cow::Borrowed("gemini-2.5-flash-lite-preview-06-17")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview092025) => {
                Cow::Borrowed("gemini-2.5-flash-lite-preview-09-2025")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0417) => {
                Cow::Borrowed("gemini-2.5-flash-preview-04-17")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0520) => {
                Cow::Borrowed("gemini-2.5-flash-preview-05-20")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview092025) => {
                Cow::Borrowed("gemini-2.5-flash-preview-09-2025")
            }
            LlmModel::Gemini(GeminiModel::Gemini25Pro) => Cow::Borrowed("gemini-2.5-pro"),
            LlmModel::Gemini(GeminiModel::Gemini25ProPreview0506) => {
                Cow::Borrowed("gemini-2.5-pro-preview-05-06")
            }
            LlmModel::Gemini(GeminiModel::Gemini25ProPreview0605) => {
                Cow::Borrowed("gemini-2.5-pro-preview-06-05")
            }
            LlmModel::Gemini(GeminiModel::Gemini3FlashPreview) => {
                Cow::Borrowed("gemini-3-flash-preview")
            }
            LlmModel::Gemini(GeminiModel::Gemini3ProPreview) => {
                Cow::Borrowed("gemini-3-pro-preview")
            }
            LlmModel::Gemini(GeminiModel::GeminiLive25Flash) => {
                Cow::Borrowed("gemini-live-2.5-flash")
            }
            LlmModel::Gemini(GeminiModel::GeminiLive25FlashPreviewNativeAudio) => {
                Cow::Borrowed("gemini-live-2.5-flash-preview-native-audio")
            }
            LlmModel::Moonshot(MoonshotModel::KimiK20711Preview) => {
                Cow::Borrowed("kimi-k2-0711-preview")
            }
            LlmModel::Moonshot(MoonshotModel::KimiK20905Preview) => {
                Cow::Borrowed("kimi-k2-0905-preview")
            }
            LlmModel::Moonshot(MoonshotModel::KimiK2Thinking) => Cow::Borrowed("kimi-k2-thinking"),
            LlmModel::Moonshot(MoonshotModel::KimiK2ThinkingTurbo) => {
                Cow::Borrowed("kimi-k2-thinking-turbo")
            }
            LlmModel::Moonshot(MoonshotModel::KimiK2TurboPreview) => {
                Cow::Borrowed("kimi-k2-turbo-preview")
            }
            LlmModel::Moonshot(MoonshotModel::KimiK25) => Cow::Borrowed("kimi-k2.5"),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude35Haiku) => {
                Cow::Borrowed("anthropic/claude-3.5-haiku")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude37Sonnet) => {
                Cow::Borrowed("anthropic/claude-3.7-sonnet")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeHaiku45) => {
                Cow::Borrowed("anthropic/claude-haiku-4.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus4) => {
                Cow::Borrowed("anthropic/claude-opus-4")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus41) => {
                Cow::Borrowed("anthropic/claude-opus-4.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus45) => {
                Cow::Borrowed("anthropic/claude-opus-4.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus46) => {
                Cow::Borrowed("anthropic/claude-opus-4.6")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet4) => {
                Cow::Borrowed("anthropic/claude-sonnet-4")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet45) => {
                Cow::Borrowed("anthropic/claude-sonnet-4.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityLargePreviewFree) => {
                Cow::Borrowed("arcee-ai/trinity-large-preview:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityMiniFree) => {
                Cow::Borrowed("arcee-ai/trinity-mini:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30Mistral24b) => {
                Cow::Borrowed("cognitivecomputations/dolphin3.0-mistral-24b")
            }
            LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30R1Mistral24b) => {
                Cow::Borrowed("cognitivecomputations/dolphin3.0-r1-mistral-24b")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekChatV31) => {
                Cow::Borrowed("deepseek/deepseek-chat-v3.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR10528Qwen38bFree) => {
                Cow::Borrowed("deepseek/deepseek-r1-0528-qwen3-8b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR1Free) => {
                Cow::Borrowed("deepseek/deepseek-r1:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31Terminus) => {
                Cow::Borrowed("deepseek/deepseek-v3.1-terminus")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31TerminusExacto) => {
                Cow::Borrowed("deepseek/deepseek-v3.1-terminus:exacto")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32) => {
                Cow::Borrowed("deepseek/deepseek-v3.2")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32Speciale) => {
                Cow::Borrowed("deepseek/deepseek-v3.2-speciale")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20Flash001) => {
                Cow::Borrowed("google/gemini-2.0-flash-001")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20FlashExpFree) => {
                Cow::Borrowed("google/gemini-2.0-flash-exp:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Flash) => {
                Cow::Borrowed("google/gemini-2.5-flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLite) => {
                Cow::Borrowed("google/gemini-2.5-flash-lite")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLitePreview092025) => {
                Cow::Borrowed("google/gemini-2.5-flash-lite-preview-09-2025")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashPreview092025) => {
                Cow::Borrowed("google/gemini-2.5-flash-preview-09-2025")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Pro) => {
                Cow::Borrowed("google/gemini-2.5-pro")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0506) => {
                Cow::Borrowed("google/gemini-2.5-pro-preview-05-06")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0605) => {
                Cow::Borrowed("google/gemini-2.5-pro-preview-06-05")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3FlashPreview) => {
                Cow::Borrowed("google/gemini-3-flash-preview")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3ProPreview) => {
                Cow::Borrowed("google/gemini-3-pro-preview")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bIt) => {
                Cow::Borrowed("google/gemma-3-27b-it")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bItFree) => {
                Cow::Borrowed("google/gemma-3-27b-it:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::KwaipilotKatCoderProFree) => {
                Cow::Borrowed("kwaipilot/kat-coder-pro:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama3370bInstructFree) => {
                Cow::Borrowed("meta-llama/llama-3.3-70b-instruct:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama4ScoutFree) => {
                Cow::Borrowed("meta-llama/llama-4-scout:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MicrosoftMaiDsR1Free) => {
                Cow::Borrowed("microsoft/mai-ds-r1:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimax01) => {
                Cow::Borrowed("minimax/minimax-01")
            }
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM1) => {
                Cow::Borrowed("minimax/minimax-m1")
            }
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM2) => {
                Cow::Borrowed("minimax/minimax-m2")
            }
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM21) => {
                Cow::Borrowed("minimax/minimax-m2.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM25) => {
                Cow::Borrowed("minimax/minimax-m2.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiCodestral2508) => {
                Cow::Borrowed("mistralai/codestral-2508")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512) => {
                Cow::Borrowed("mistralai/devstral-2512")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512Free) => {
                Cow::Borrowed("mistralai/devstral-2512:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralMedium2507) => {
                Cow::Borrowed("mistralai/devstral-medium-2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505) => {
                Cow::Borrowed("mistralai/devstral-small-2505")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505Free) => {
                Cow::Borrowed("mistralai/devstral-small-2505:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2507) => {
                Cow::Borrowed("mistralai/devstral-small-2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistral7bInstructFree) => {
                Cow::Borrowed("mistralai/mistral-7b-instruct:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium3) => {
                Cow::Borrowed("mistralai/mistral-medium-3")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium31) => {
                Cow::Borrowed("mistralai/mistral-medium-3.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralNemoFree) => {
                Cow::Borrowed("mistralai/mistral-nemo:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3124bInstruct) => {
                Cow::Borrowed("mistralai/mistral-small-3.1-24b-instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstruct) => {
                Cow::Borrowed("mistralai/mistral-small-3.2-24b-instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstructFree) => {
                Cow::Borrowed("mistralai/mistral-small-3.2-24b-instruct:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiDev72bFree) => {
                Cow::Borrowed("moonshotai/kimi-dev-72b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2) => {
                Cow::Borrowed("moonshotai/kimi-k2")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905) => {
                Cow::Borrowed("moonshotai/kimi-k2-0905")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905Exacto) => {
                Cow::Borrowed("moonshotai/kimi-k2-0905:exacto")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Thinking) => {
                Cow::Borrowed("moonshotai/kimi-k2-thinking")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK25) => {
                Cow::Borrowed("moonshotai/kimi-k2.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Free) => {
                Cow::Borrowed("moonshotai/kimi-k2:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::NousresearchDeephermes3Llama38bPreview) => {
                Cow::Borrowed("nousresearch/deephermes-3-llama-3-8b-preview")
            }
            LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes4405b) => {
                Cow::Borrowed("nousresearch/hermes-4-405b")
            }
            LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes470b) => {
                Cow::Borrowed("nousresearch/hermes-4-70b")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotron3Nano30bA3bFree) => {
                Cow::Borrowed("nvidia/nemotron-3-nano-30b-a3b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano12bV2VlFree) => {
                Cow::Borrowed("nvidia/nemotron-nano-12b-v2-vl:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2) => {
                Cow::Borrowed("nvidia/nemotron-nano-9b-v2")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2Free) => {
                Cow::Borrowed("nvidia/nemotron-nano-9b-v2:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41) => Cow::Borrowed("openai/gpt-4.1"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41Mini) => {
                Cow::Borrowed("openai/gpt-4.1-mini")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt4oMini) => {
                Cow::Borrowed("openai/gpt-4o-mini")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5) => Cow::Borrowed("openai/gpt-5"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Codex) => {
                Cow::Borrowed("openai/gpt-5-codex")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Image) => {
                Cow::Borrowed("openai/gpt-5-image")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Mini) => {
                Cow::Borrowed("openai/gpt-5-mini")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Nano) => {
                Cow::Borrowed("openai/gpt-5-nano")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Pro) => {
                Cow::Borrowed("openai/gpt-5-pro")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51) => Cow::Borrowed("openai/gpt-5.1"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Chat) => {
                Cow::Borrowed("openai/gpt-5.1-chat")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Codex) => {
                Cow::Borrowed("openai/gpt-5.1-codex")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMax) => {
                Cow::Borrowed("openai/gpt-5.1-codex-max")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMini) => {
                Cow::Borrowed("openai/gpt-5.1-codex-mini")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52) => Cow::Borrowed("openai/gpt-5.2"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Chat) => {
                Cow::Borrowed("openai/gpt-5.2-chat")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Codex) => {
                Cow::Borrowed("openai/gpt-5.2-codex")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Pro) => {
                Cow::Borrowed("openai/gpt-5.2-pro")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120b) => {
                Cow::Borrowed("openai/gpt-oss-120b")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bExacto) => {
                Cow::Borrowed("openai/gpt-oss-120b:exacto")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bFree) => {
                Cow::Borrowed("openai/gpt-oss-120b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20b) => {
                Cow::Borrowed("openai/gpt-oss-20b")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20bFree) => {
                Cow::Borrowed("openai/gpt-oss-20b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOssSafeguard20b) => {
                Cow::Borrowed("openai/gpt-oss-safeguard-20b")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiO4Mini) => Cow::Borrowed("openai/o4-mini"),
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterAuroraAlpha) => {
                Cow::Borrowed("openrouter/aurora-alpha")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockDashAlpha) => {
                Cow::Borrowed("openrouter/sherlock-dash-alpha")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockThinkAlpha) => {
                Cow::Borrowed("openrouter/sherlock-think-alpha")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl7bInstructFree) => {
                Cow::Borrowed("qwen/qwen-2.5-vl-7b-instruct:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl32bInstructFree) => {
                Cow::Borrowed("qwen/qwen2.5-vl-32b-instruct:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl72bInstructFree) => {
                Cow::Borrowed("qwen/qwen2.5-vl-72b-instruct:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen314bFree) => {
                Cow::Borrowed("qwen/qwen3-14b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725) => {
                Cow::Borrowed("qwen/qwen3-235b-a22b-07-25")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725Free) => {
                Cow::Borrowed("qwen/qwen3-235b-a22b-07-25:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bThinking2507) => {
                Cow::Borrowed("qwen/qwen3-235b-a22b-thinking-2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bFree) => {
                Cow::Borrowed("qwen/qwen3-235b-a22b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bInstruct2507) => {
                Cow::Borrowed("qwen/qwen3-30b-a3b-instruct-2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bThinking2507) => {
                Cow::Borrowed("qwen/qwen3-30b-a3b-thinking-2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bFree) => {
                Cow::Borrowed("qwen/qwen3-30b-a3b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen332bFree) => {
                Cow::Borrowed("qwen/qwen3-32b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen34bFree) => {
                Cow::Borrowed("qwen/qwen3-4b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen38bFree) => {
                Cow::Borrowed("qwen/qwen3-8b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder) => {
                Cow::Borrowed("qwen/qwen3-coder")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder30bA3bInstruct) => {
                Cow::Borrowed("qwen/qwen3-coder-30b-a3b-instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFlash) => {
                Cow::Borrowed("qwen/qwen3-coder-flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderExacto) => {
                Cow::Borrowed("qwen/qwen3-coder:exacto")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFree) => {
                Cow::Borrowed("qwen/qwen3-coder:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Max) => Cow::Borrowed("qwen/qwen3-max"),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstruct) => {
                Cow::Borrowed("qwen/qwen3-next-80b-a3b-instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstructFree) => {
                Cow::Borrowed("qwen/qwen3-next-80b-a3b-instruct:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bThinking) => {
                Cow::Borrowed("qwen/qwen3-next-80b-a3b-thinking")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwq32bFree) => {
                Cow::Borrowed("qwen/qwq-32b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::RekaaiRekaFlash3) => {
                Cow::Borrowed("rekaai/reka-flash-3")
            }
            LlmModel::OpenRouter(OpenRouterModel::SarvamaiSarvamMFree) => {
                Cow::Borrowed("sarvamai/sarvam-m:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::StepfunStep35Flash) => {
                Cow::Borrowed("stepfun/step-3.5-flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::StepfunStep35FlashFree) => {
                Cow::Borrowed("stepfun/step-3.5-flash:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::ThudmGlmZ132bFree) => {
                Cow::Borrowed("thudm/glm-z1-32b:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::TngtechTngR1tChimeraFree) => {
                Cow::Borrowed("tngtech/tng-r1t-chimera:free")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3) => Cow::Borrowed("x-ai/grok-3"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Beta) => {
                Cow::Borrowed("x-ai/grok-3-beta")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Mini) => {
                Cow::Borrowed("x-ai/grok-3-mini")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3MiniBeta) => {
                Cow::Borrowed("x-ai/grok-3-mini-beta")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok4) => Cow::Borrowed("x-ai/grok-4"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok4Fast) => {
                Cow::Borrowed("x-ai/grok-4-fast")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok41Fast) => {
                Cow::Borrowed("x-ai/grok-4.1-fast")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrokCodeFast1) => {
                Cow::Borrowed("x-ai/grok-code-fast-1")
            }
            LlmModel::OpenRouter(OpenRouterModel::XiaomiMimoV2Flash) => {
                Cow::Borrowed("xiaomi/mimo-v2-flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45) => Cow::Borrowed("z-ai/glm-4.5"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45Air) => Cow::Borrowed("z-ai/glm-4.5-air"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45v) => Cow::Borrowed("z-ai/glm-4.5v"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46) => Cow::Borrowed("z-ai/glm-4.6"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46Exacto) => {
                Cow::Borrowed("z-ai/glm-4.6:exacto")
            }
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47) => Cow::Borrowed("z-ai/glm-4.7"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47Flash) => {
                Cow::Borrowed("z-ai/glm-4.7-flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm5) => Cow::Borrowed("z-ai/glm-5"),
            LlmModel::ZAi(ZAiModel::Glm45) => Cow::Borrowed("glm-4.5"),
            LlmModel::ZAi(ZAiModel::Glm45Air) => Cow::Borrowed("glm-4.5-air"),
            LlmModel::ZAi(ZAiModel::Glm45Flash) => Cow::Borrowed("glm-4.5-flash"),
            LlmModel::ZAi(ZAiModel::Glm45v) => Cow::Borrowed("glm-4.5v"),
            LlmModel::ZAi(ZAiModel::Glm46) => Cow::Borrowed("glm-4.6"),
            LlmModel::ZAi(ZAiModel::Glm46v) => Cow::Borrowed("glm-4.6v"),
            LlmModel::ZAi(ZAiModel::Glm47) => Cow::Borrowed("glm-4.7"),
            LlmModel::ZAi(ZAiModel::Glm47Flash) => Cow::Borrowed("glm-4.7-flash"),
            LlmModel::ZAi(ZAiModel::Glm5) => Cow::Borrowed("glm-5"),
            LlmModel::Ollama(s) => Cow::Owned(s.clone()),
            LlmModel::LlamaCpp(s) => Cow::Owned(s.clone()),
        }
    }

    /// Human-readable display name (e.g. "Claude Opus 4.6")
    pub fn display_name(&self) -> Cow<'static, str> {
        match self {
            LlmModel::Anthropic(AnthropicModel::Claude35Haiku20241022) => {
                Cow::Borrowed("Claude Haiku 3.5")
            }
            LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20240620) => {
                Cow::Borrowed("Claude Sonnet 3.5")
            }
            LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20241022) => {
                Cow::Borrowed("Claude Sonnet 3.5 v2")
            }
            LlmModel::Anthropic(AnthropicModel::Claude37Sonnet20250219) => {
                Cow::Borrowed("Claude Sonnet 3.7")
            }
            LlmModel::Anthropic(AnthropicModel::Claude3Haiku20240307) => {
                Cow::Borrowed("Claude Haiku 3")
            }
            LlmModel::Anthropic(AnthropicModel::Claude3Opus20240229) => {
                Cow::Borrowed("Claude Opus 3")
            }
            LlmModel::Anthropic(AnthropicModel::Claude3Sonnet20240229) => {
                Cow::Borrowed("Claude Sonnet 3")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeHaiku45) => {
                Cow::Borrowed("Claude Haiku 4.5 (latest)")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeHaiku4520251001) => {
                Cow::Borrowed("Claude Haiku 4.5")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus40) => {
                Cow::Borrowed("Claude Opus 4 (latest)")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus41) => {
                Cow::Borrowed("Claude Opus 4.1 (latest)")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus4120250805) => {
                Cow::Borrowed("Claude Opus 4.1")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus420250514) => {
                Cow::Borrowed("Claude Opus 4")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus45) => {
                Cow::Borrowed("Claude Opus 4.5 (latest)")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus4520251101) => {
                Cow::Borrowed("Claude Opus 4.5")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus46) => Cow::Borrowed("Claude Opus 4.6"),
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet40) => {
                Cow::Borrowed("Claude Sonnet 4 (latest)")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet420250514) => {
                Cow::Borrowed("Claude Sonnet 4")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet45) => {
                Cow::Borrowed("Claude Sonnet 4.5 (latest)")
            }
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet4520250929) => {
                Cow::Borrowed("Claude Sonnet 4.5")
            }
            LlmModel::DeepSeek(DeepSeekModel::DeepseekChat) => Cow::Borrowed("DeepSeek Chat"),
            LlmModel::DeepSeek(DeepSeekModel::DeepseekReasoner) => {
                Cow::Borrowed("DeepSeek Reasoner")
            }
            LlmModel::Gemini(GeminiModel::Gemini15Flash) => Cow::Borrowed("Gemini 1.5 Flash"),
            LlmModel::Gemini(GeminiModel::Gemini15Flash8b) => Cow::Borrowed("Gemini 1.5 Flash-8B"),
            LlmModel::Gemini(GeminiModel::Gemini15Pro) => Cow::Borrowed("Gemini 1.5 Pro"),
            LlmModel::Gemini(GeminiModel::Gemini20Flash) => Cow::Borrowed("Gemini 2.0 Flash"),
            LlmModel::Gemini(GeminiModel::Gemini20FlashLite) => {
                Cow::Borrowed("Gemini 2.0 Flash Lite")
            }
            LlmModel::Gemini(GeminiModel::Gemini25Flash) => Cow::Borrowed("Gemini 2.5 Flash"),
            LlmModel::Gemini(GeminiModel::Gemini25FlashLite) => {
                Cow::Borrowed("Gemini 2.5 Flash Lite")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview0617) => {
                Cow::Borrowed("Gemini 2.5 Flash Lite Preview 06-17")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview092025) => {
                Cow::Borrowed("Gemini 2.5 Flash Lite Preview 09-25")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0417) => {
                Cow::Borrowed("Gemini 2.5 Flash Preview 04-17")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0520) => {
                Cow::Borrowed("Gemini 2.5 Flash Preview 05-20")
            }
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview092025) => {
                Cow::Borrowed("Gemini 2.5 Flash Preview 09-25")
            }
            LlmModel::Gemini(GeminiModel::Gemini25Pro) => Cow::Borrowed("Gemini 2.5 Pro"),
            LlmModel::Gemini(GeminiModel::Gemini25ProPreview0506) => {
                Cow::Borrowed("Gemini 2.5 Pro Preview 05-06")
            }
            LlmModel::Gemini(GeminiModel::Gemini25ProPreview0605) => {
                Cow::Borrowed("Gemini 2.5 Pro Preview 06-05")
            }
            LlmModel::Gemini(GeminiModel::Gemini3FlashPreview) => {
                Cow::Borrowed("Gemini 3 Flash Preview")
            }
            LlmModel::Gemini(GeminiModel::Gemini3ProPreview) => {
                Cow::Borrowed("Gemini 3 Pro Preview")
            }
            LlmModel::Gemini(GeminiModel::GeminiLive25Flash) => {
                Cow::Borrowed("Gemini Live 2.5 Flash")
            }
            LlmModel::Gemini(GeminiModel::GeminiLive25FlashPreviewNativeAudio) => {
                Cow::Borrowed("Gemini Live 2.5 Flash Preview Native Audio")
            }
            LlmModel::Moonshot(MoonshotModel::KimiK20711Preview) => Cow::Borrowed("Kimi K2 0711"),
            LlmModel::Moonshot(MoonshotModel::KimiK20905Preview) => Cow::Borrowed("Kimi K2 0905"),
            LlmModel::Moonshot(MoonshotModel::KimiK2Thinking) => Cow::Borrowed("Kimi K2 Thinking"),
            LlmModel::Moonshot(MoonshotModel::KimiK2ThinkingTurbo) => {
                Cow::Borrowed("Kimi K2 Thinking Turbo")
            }
            LlmModel::Moonshot(MoonshotModel::KimiK2TurboPreview) => Cow::Borrowed("Kimi K2 Turbo"),
            LlmModel::Moonshot(MoonshotModel::KimiK25) => Cow::Borrowed("Kimi K2.5"),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude35Haiku) => {
                Cow::Borrowed("Claude Haiku 3.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude37Sonnet) => {
                Cow::Borrowed("Claude Sonnet 3.7")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeHaiku45) => {
                Cow::Borrowed("Claude Haiku 4.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus4) => {
                Cow::Borrowed("Claude Opus 4")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus41) => {
                Cow::Borrowed("Claude Opus 4.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus45) => {
                Cow::Borrowed("Claude Opus 4.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus46) => {
                Cow::Borrowed("Claude Opus 4.6")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet4) => {
                Cow::Borrowed("Claude Sonnet 4")
            }
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet45) => {
                Cow::Borrowed("Claude Sonnet 4.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityLargePreviewFree) => {
                Cow::Borrowed("Trinity Large Preview")
            }
            LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityMiniFree) => {
                Cow::Borrowed("Trinity Mini")
            }
            LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30Mistral24b) => {
                Cow::Borrowed("Dolphin3.0 Mistral 24B")
            }
            LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30R1Mistral24b) => {
                Cow::Borrowed("Dolphin3.0 R1 Mistral 24B")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekChatV31) => {
                Cow::Borrowed("DeepSeek-V3.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR10528Qwen38bFree) => {
                Cow::Borrowed("Deepseek R1 0528 Qwen3 8B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR1Free) => {
                Cow::Borrowed("R1 (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31Terminus) => {
                Cow::Borrowed("DeepSeek V3.1 Terminus")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31TerminusExacto) => {
                Cow::Borrowed("DeepSeek V3.1 Terminus (exacto)")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32) => {
                Cow::Borrowed("DeepSeek V3.2")
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32Speciale) => {
                Cow::Borrowed("DeepSeek V3.2 Speciale")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20Flash001) => {
                Cow::Borrowed("Gemini 2.0 Flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20FlashExpFree) => {
                Cow::Borrowed("Gemini 2.0 Flash Experimental (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Flash) => {
                Cow::Borrowed("Gemini 2.5 Flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLite) => {
                Cow::Borrowed("Gemini 2.5 Flash Lite")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLitePreview092025) => {
                Cow::Borrowed("Gemini 2.5 Flash Lite Preview 09-25")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashPreview092025) => {
                Cow::Borrowed("Gemini 2.5 Flash Preview 09-25")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Pro) => {
                Cow::Borrowed("Gemini 2.5 Pro")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0506) => {
                Cow::Borrowed("Gemini 2.5 Pro Preview 05-06")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0605) => {
                Cow::Borrowed("Gemini 2.5 Pro Preview 06-05")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3FlashPreview) => {
                Cow::Borrowed("Gemini 3 Flash Preview")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3ProPreview) => {
                Cow::Borrowed("Gemini 3 Pro Preview")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bIt) => {
                Cow::Borrowed("Gemma 3 27B")
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bItFree) => {
                Cow::Borrowed("Gemma 3 27B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::KwaipilotKatCoderProFree) => {
                Cow::Borrowed("Kat Coder Pro (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama3370bInstructFree) => {
                Cow::Borrowed("Llama 3.3 70B Instruct (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama4ScoutFree) => {
                Cow::Borrowed("Llama 4 Scout (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MicrosoftMaiDsR1Free) => {
                Cow::Borrowed("MAI DS R1 (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimax01) => Cow::Borrowed("MiniMax-01"),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM1) => Cow::Borrowed("MiniMax M1"),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM2) => Cow::Borrowed("MiniMax M2"),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM21) => {
                Cow::Borrowed("MiniMax M2.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM25) => {
                Cow::Borrowed("MiniMax M2.5")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiCodestral2508) => {
                Cow::Borrowed("Codestral 2508")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512) => {
                Cow::Borrowed("Devstral 2 2512")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512Free) => {
                Cow::Borrowed("Devstral 2 2512 (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralMedium2507) => {
                Cow::Borrowed("Devstral Medium")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505) => {
                Cow::Borrowed("Devstral Small")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505Free) => {
                Cow::Borrowed("Devstral Small 2505 (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2507) => {
                Cow::Borrowed("Devstral Small 1.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistral7bInstructFree) => {
                Cow::Borrowed("Mistral 7B Instruct (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium3) => {
                Cow::Borrowed("Mistral Medium 3")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium31) => {
                Cow::Borrowed("Mistral Medium 3.1")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralNemoFree) => {
                Cow::Borrowed("Mistral Nemo (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3124bInstruct) => {
                Cow::Borrowed("Mistral Small 3.1 24B Instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstruct) => {
                Cow::Borrowed("Mistral Small 3.2 24B Instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstructFree) => {
                Cow::Borrowed("Mistral Small 3.2 24B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiDev72bFree) => {
                Cow::Borrowed("Kimi Dev 72b (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2) => Cow::Borrowed("Kimi K2"),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905) => {
                Cow::Borrowed("Kimi K2 Instruct 0905")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905Exacto) => {
                Cow::Borrowed("Kimi K2 Instruct 0905 (exacto)")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Thinking) => {
                Cow::Borrowed("Kimi K2 Thinking")
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK25) => Cow::Borrowed("Kimi K2.5"),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Free) => {
                Cow::Borrowed("Kimi K2 (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::NousresearchDeephermes3Llama38bPreview) => {
                Cow::Borrowed("DeepHermes 3 Llama 3 8B Preview")
            }
            LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes4405b) => {
                Cow::Borrowed("Hermes 4 405B")
            }
            LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes470b) => {
                Cow::Borrowed("Hermes 4 70B")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotron3Nano30bA3bFree) => {
                Cow::Borrowed("Nemotron 3 Nano 30B A3B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano12bV2VlFree) => {
                Cow::Borrowed("Nemotron Nano 12B 2 VL (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2) => {
                Cow::Borrowed("nvidia-nemotron-nano-9b-v2")
            }
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2Free) => {
                Cow::Borrowed("Nemotron Nano 9B V2 (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41) => Cow::Borrowed("GPT-4.1"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41Mini) => Cow::Borrowed("GPT-4.1 Mini"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt4oMini) => Cow::Borrowed("GPT-4o-mini"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5) => Cow::Borrowed("GPT-5"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Codex) => Cow::Borrowed("GPT-5 Codex"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Image) => Cow::Borrowed("GPT-5 Image"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Mini) => Cow::Borrowed("GPT-5 Mini"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Nano) => Cow::Borrowed("GPT-5 Nano"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Pro) => Cow::Borrowed("GPT-5 Pro"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51) => Cow::Borrowed("GPT-5.1"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Chat) => Cow::Borrowed("GPT-5.1 Chat"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Codex) => {
                Cow::Borrowed("GPT-5.1-Codex")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMax) => {
                Cow::Borrowed("GPT-5.1-Codex-Max")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMini) => {
                Cow::Borrowed("GPT-5.1-Codex-Mini")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52) => Cow::Borrowed("GPT-5.2"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Chat) => Cow::Borrowed("GPT-5.2 Chat"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Codex) => {
                Cow::Borrowed("GPT-5.2-Codex")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Pro) => Cow::Borrowed("GPT-5.2 Pro"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120b) => {
                Cow::Borrowed("GPT OSS 120B")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bExacto) => {
                Cow::Borrowed("GPT OSS 120B (exacto)")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bFree) => {
                Cow::Borrowed("gpt-oss-120b (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20b) => Cow::Borrowed("GPT OSS 20B"),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20bFree) => {
                Cow::Borrowed("gpt-oss-20b (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOssSafeguard20b) => {
                Cow::Borrowed("GPT OSS Safeguard 20B")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenaiO4Mini) => Cow::Borrowed("o4 Mini"),
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterAuroraAlpha) => {
                Cow::Borrowed("Aurora Alpha")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockDashAlpha) => {
                Cow::Borrowed("Sherlock Dash Alpha")
            }
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockThinkAlpha) => {
                Cow::Borrowed("Sherlock Think Alpha")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl7bInstructFree) => {
                Cow::Borrowed("Qwen2.5-VL 7B Instruct (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl32bInstructFree) => {
                Cow::Borrowed("Qwen2.5 VL 32B Instruct (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl72bInstructFree) => {
                Cow::Borrowed("Qwen2.5 VL 72B Instruct (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen314bFree) => {
                Cow::Borrowed("Qwen3 14B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725) => {
                Cow::Borrowed("Qwen3 235B A22B Instruct 2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725Free) => {
                Cow::Borrowed("Qwen3 235B A22B Instruct 2507 (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bThinking2507) => {
                Cow::Borrowed("Qwen3 235B A22B Thinking 2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bFree) => {
                Cow::Borrowed("Qwen3 235B A22B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bInstruct2507) => {
                Cow::Borrowed("Qwen3 30B A3B Instruct 2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bThinking2507) => {
                Cow::Borrowed("Qwen3 30B A3B Thinking 2507")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bFree) => {
                Cow::Borrowed("Qwen3 30B A3B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen332bFree) => {
                Cow::Borrowed("Qwen3 32B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen34bFree) => {
                Cow::Borrowed("Qwen3 4B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen38bFree) => {
                Cow::Borrowed("Qwen3 8B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder) => Cow::Borrowed("Qwen3 Coder"),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder30bA3bInstruct) => {
                Cow::Borrowed("Qwen3 Coder 30B A3B Instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFlash) => {
                Cow::Borrowed("Qwen3 Coder Flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderExacto) => {
                Cow::Borrowed("Qwen3 Coder (exacto)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFree) => {
                Cow::Borrowed("Qwen3 Coder 480B A35B Instruct (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Max) => Cow::Borrowed("Qwen3 Max"),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstruct) => {
                Cow::Borrowed("Qwen3 Next 80B A3B Instruct")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstructFree) => {
                Cow::Borrowed("Qwen3 Next 80B A3B Instruct (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bThinking) => {
                Cow::Borrowed("Qwen3 Next 80B A3B Thinking")
            }
            LlmModel::OpenRouter(OpenRouterModel::QwenQwq32bFree) => {
                Cow::Borrowed("QwQ 32B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::RekaaiRekaFlash3) => {
                Cow::Borrowed("Reka Flash 3")
            }
            LlmModel::OpenRouter(OpenRouterModel::SarvamaiSarvamMFree) => {
                Cow::Borrowed("Sarvam-M (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::StepfunStep35Flash) => {
                Cow::Borrowed("Step 3.5 Flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::StepfunStep35FlashFree) => {
                Cow::Borrowed("Step 3.5 Flash (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::ThudmGlmZ132bFree) => {
                Cow::Borrowed("GLM Z1 32B (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::TngtechTngR1tChimeraFree) => {
                Cow::Borrowed("R1T Chimera (free)")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3) => Cow::Borrowed("Grok 3"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Beta) => Cow::Borrowed("Grok 3 Beta"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Mini) => Cow::Borrowed("Grok 3 Mini"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3MiniBeta) => {
                Cow::Borrowed("Grok 3 Mini Beta")
            }
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok4) => Cow::Borrowed("Grok 4"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok4Fast) => Cow::Borrowed("Grok 4 Fast"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok41Fast) => Cow::Borrowed("Grok 4.1 Fast"),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrokCodeFast1) => {
                Cow::Borrowed("Grok Code Fast 1")
            }
            LlmModel::OpenRouter(OpenRouterModel::XiaomiMimoV2Flash) => {
                Cow::Borrowed("MiMo-V2-Flash")
            }
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45) => Cow::Borrowed("GLM 4.5"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45Air) => Cow::Borrowed("GLM 4.5 Air"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45v) => Cow::Borrowed("GLM 4.5V"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46) => Cow::Borrowed("GLM 4.6"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46Exacto) => {
                Cow::Borrowed("GLM 4.6 (exacto)")
            }
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47) => Cow::Borrowed("GLM-4.7"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47Flash) => Cow::Borrowed("GLM-4.7"),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm5) => Cow::Borrowed("GLM-5"),
            LlmModel::ZAi(ZAiModel::Glm45) => Cow::Borrowed("GLM-4.5"),
            LlmModel::ZAi(ZAiModel::Glm45Air) => Cow::Borrowed("GLM-4.5-Air"),
            LlmModel::ZAi(ZAiModel::Glm45Flash) => Cow::Borrowed("GLM-4.5-Flash"),
            LlmModel::ZAi(ZAiModel::Glm45v) => Cow::Borrowed("GLM-4.5V"),
            LlmModel::ZAi(ZAiModel::Glm46) => Cow::Borrowed("GLM-4.6"),
            LlmModel::ZAi(ZAiModel::Glm46v) => Cow::Borrowed("GLM-4.6V"),
            LlmModel::ZAi(ZAiModel::Glm47) => Cow::Borrowed("GLM-4.7"),
            LlmModel::ZAi(ZAiModel::Glm47Flash) => Cow::Borrowed("GLM-4.7-Flash"),
            LlmModel::ZAi(ZAiModel::Glm5) => Cow::Borrowed("GLM-5"),
            LlmModel::Ollama(s) => Cow::Owned(format!("Ollama {s}")),
            LlmModel::LlamaCpp(s) => Cow::Owned(format!("LlamaCpp {s}")),
        }
    }

    /// Provider identifier (e.g. "anthropic")
    pub fn provider(&self) -> &'static str {
        match self {
            LlmModel::Anthropic(_) => "anthropic",
            LlmModel::DeepSeek(_) => "deepseek",
            LlmModel::Gemini(_) => "gemini",
            LlmModel::Moonshot(_) => "moonshot",
            LlmModel::OpenRouter(_) => "openrouter",
            LlmModel::ZAi(_) => "zai",
            LlmModel::Ollama(_) => "ollama",
            LlmModel::LlamaCpp(_) => "llamacpp",
        }
    }

    /// Context window size in tokens (None for dynamic providers)
    pub fn context_window(&self) -> Option<u32> {
        match self {
            LlmModel::Anthropic(AnthropicModel::Claude35Haiku20241022) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20240620) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20241022) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::Claude37Sonnet20250219) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::Claude3Haiku20240307) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::Claude3Opus20240229) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::Claude3Sonnet20240229) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeHaiku45) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeHaiku4520251001) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus40) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus41) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus4120250805) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus420250514) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus45) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus4520251101) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeOpus46) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet40) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet420250514) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet45) => Some(200000),
            LlmModel::Anthropic(AnthropicModel::ClaudeSonnet4520250929) => Some(200000),
            LlmModel::DeepSeek(DeepSeekModel::DeepseekChat) => Some(128000),
            LlmModel::DeepSeek(DeepSeekModel::DeepseekReasoner) => Some(128000),
            LlmModel::Gemini(GeminiModel::Gemini15Flash) => Some(1000000),
            LlmModel::Gemini(GeminiModel::Gemini15Flash8b) => Some(1000000),
            LlmModel::Gemini(GeminiModel::Gemini15Pro) => Some(1000000),
            LlmModel::Gemini(GeminiModel::Gemini20Flash) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini20FlashLite) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25Flash) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25FlashLite) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview0617) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview092025) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0417) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0520) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25FlashPreview092025) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25Pro) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25ProPreview0506) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini25ProPreview0605) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini3FlashPreview) => Some(1048576),
            LlmModel::Gemini(GeminiModel::Gemini3ProPreview) => Some(1000000),
            LlmModel::Gemini(GeminiModel::GeminiLive25Flash) => Some(128000),
            LlmModel::Gemini(GeminiModel::GeminiLive25FlashPreviewNativeAudio) => Some(131072),
            LlmModel::Moonshot(MoonshotModel::KimiK20711Preview) => Some(131072),
            LlmModel::Moonshot(MoonshotModel::KimiK20905Preview) => Some(262144),
            LlmModel::Moonshot(MoonshotModel::KimiK2Thinking) => Some(262144),
            LlmModel::Moonshot(MoonshotModel::KimiK2ThinkingTurbo) => Some(262144),
            LlmModel::Moonshot(MoonshotModel::KimiK2TurboPreview) => Some(262144),
            LlmModel::Moonshot(MoonshotModel::KimiK25) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude35Haiku) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude37Sonnet) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeHaiku45) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus4) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus41) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus45) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus46) => Some(1000000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet4) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet45) => Some(1000000),
            LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityLargePreviewFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityMiniFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30Mistral24b) => {
                Some(32768)
            }
            LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30R1Mistral24b) => {
                Some(32768)
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekChatV31) => Some(163840),
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR10528Qwen38bFree) => {
                Some(131072)
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR1Free) => Some(163840),
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31Terminus) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31TerminusExacto) => {
                Some(131072)
            }
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32) => Some(163840),
            LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32Speciale) => Some(163840),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20Flash001) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20FlashExpFree) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Flash) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLite) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLitePreview092025) => {
                Some(1048576)
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashPreview092025) => {
                Some(1048576)
            }
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Pro) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0506) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0605) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3FlashPreview) => Some(1048576),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3ProPreview) => Some(1050000),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bIt) => Some(96000),
            LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bItFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::KwaipilotKatCoderProFree) => Some(256000),
            LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama3370bInstructFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama4ScoutFree) => Some(64000),
            LlmModel::OpenRouter(OpenRouterModel::MicrosoftMaiDsR1Free) => Some(163840),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimax01) => Some(1000000),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM1) => Some(1000000),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM2) => Some(196600),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM21) => Some(204800),
            LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM25) => Some(204800),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiCodestral2508) => Some(256000),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512Free) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralMedium2507) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505Free) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2507) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistral7bInstructFree) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium3) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium31) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralNemoFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3124bInstruct) => {
                Some(128000)
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstruct) => {
                Some(96000)
            }
            LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstructFree) => {
                Some(96000)
            }
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiDev72bFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905Exacto) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Thinking) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK25) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Free) => Some(32800),
            LlmModel::OpenRouter(OpenRouterModel::NousresearchDeephermes3Llama38bPreview) => {
                Some(131072)
            }
            LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes4405b) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes470b) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotron3Nano30bA3bFree) => Some(256000),
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano12bV2VlFree) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2Free) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41) => Some(1047576),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41Mini) => Some(1047576),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt4oMini) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Codex) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Image) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Mini) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Nano) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Pro) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Chat) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Codex) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMax) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMini) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Chat) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Codex) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Pro) => Some(400000),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120b) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bExacto) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20b) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20bFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOssSafeguard20b) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::OpenaiO4Mini) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterAuroraAlpha) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockDashAlpha) => Some(1840000),
            LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockThinkAlpha) => Some(1840000),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl7bInstructFree) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl32bInstructFree) => Some(8192),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl72bInstructFree) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen314bFree) => Some(40960),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725Free) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bThinking2507) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bFree) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bInstruct2507) => Some(262000),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bThinking2507) => Some(262000),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bFree) => Some(40960),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen332bFree) => Some(40960),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen34bFree) => Some(40960),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen38bFree) => Some(40960),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder30bA3bInstruct) => Some(160000),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFlash) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderExacto) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFree) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Max) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstruct) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstructFree) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bThinking) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::QwenQwq32bFree) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::RekaaiRekaFlash3) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::SarvamaiSarvamMFree) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::StepfunStep35Flash) => Some(256000),
            LlmModel::OpenRouter(OpenRouterModel::StepfunStep35FlashFree) => Some(256000),
            LlmModel::OpenRouter(OpenRouterModel::ThudmGlmZ132bFree) => Some(32768),
            LlmModel::OpenRouter(OpenRouterModel::TngtechTngR1tChimeraFree) => Some(163840),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Beta) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Mini) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok3MiniBeta) => Some(131072),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok4) => Some(256000),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok4Fast) => Some(2000000),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrok41Fast) => Some(2000000),
            LlmModel::OpenRouter(OpenRouterModel::XAiGrokCodeFast1) => Some(256000),
            LlmModel::OpenRouter(OpenRouterModel::XiaomiMimoV2Flash) => Some(262144),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45Air) => Some(128000),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45v) => Some(64000),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46Exacto) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47) => Some(204800),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47Flash) => Some(200000),
            LlmModel::OpenRouter(OpenRouterModel::ZAiGlm5) => Some(202752),
            LlmModel::ZAi(ZAiModel::Glm45) => Some(131072),
            LlmModel::ZAi(ZAiModel::Glm45Air) => Some(131072),
            LlmModel::ZAi(ZAiModel::Glm45Flash) => Some(131072),
            LlmModel::ZAi(ZAiModel::Glm45v) => Some(64000),
            LlmModel::ZAi(ZAiModel::Glm46) => Some(204800),
            LlmModel::ZAi(ZAiModel::Glm46v) => Some(128000),
            LlmModel::ZAi(ZAiModel::Glm47) => Some(204800),
            LlmModel::ZAi(ZAiModel::Glm47Flash) => Some(200000),
            LlmModel::ZAi(ZAiModel::Glm5) => Some(204800),
            LlmModel::Ollama(_) => None,
            LlmModel::LlamaCpp(_) => None,
        }
    }

    /// Required env var for this model's provider (None for local providers)
    pub fn required_env_var(&self) -> Option<&'static str> {
        match self {
            LlmModel::Anthropic(_) => Some("ANTHROPIC_API_KEY"),
            LlmModel::DeepSeek(_) => Some("DEEPSEEK_API_KEY"),
            LlmModel::Gemini(_) => Some("GEMINI_API_KEY"),
            LlmModel::Moonshot(_) => Some("MOONSHOT_API_KEY"),
            LlmModel::OpenRouter(_) => Some("OPENROUTER_API_KEY"),
            LlmModel::ZAi(_) => Some("ZAI_API_KEY"),
            LlmModel::Ollama(_) => None,
            LlmModel::LlamaCpp(_) => None,
        }
    }

    /// All catalog models (excludes dynamic providers)
    pub fn all() -> &'static [LlmModel] {
        static ALL: LazyLock<Vec<LlmModel>> = LazyLock::new(|| {
            vec![
                LlmModel::Anthropic(AnthropicModel::Claude35Haiku20241022),
                LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20240620),
                LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20241022),
                LlmModel::Anthropic(AnthropicModel::Claude37Sonnet20250219),
                LlmModel::Anthropic(AnthropicModel::Claude3Haiku20240307),
                LlmModel::Anthropic(AnthropicModel::Claude3Opus20240229),
                LlmModel::Anthropic(AnthropicModel::Claude3Sonnet20240229),
                LlmModel::Anthropic(AnthropicModel::ClaudeHaiku45),
                LlmModel::Anthropic(AnthropicModel::ClaudeHaiku4520251001),
                LlmModel::Anthropic(AnthropicModel::ClaudeOpus40),
                LlmModel::Anthropic(AnthropicModel::ClaudeOpus41),
                LlmModel::Anthropic(AnthropicModel::ClaudeOpus4120250805),
                LlmModel::Anthropic(AnthropicModel::ClaudeOpus420250514),
                LlmModel::Anthropic(AnthropicModel::ClaudeOpus45),
                LlmModel::Anthropic(AnthropicModel::ClaudeOpus4520251101),
                LlmModel::Anthropic(AnthropicModel::ClaudeOpus46),
                LlmModel::Anthropic(AnthropicModel::ClaudeSonnet40),
                LlmModel::Anthropic(AnthropicModel::ClaudeSonnet420250514),
                LlmModel::Anthropic(AnthropicModel::ClaudeSonnet45),
                LlmModel::Anthropic(AnthropicModel::ClaudeSonnet4520250929),
                LlmModel::DeepSeek(DeepSeekModel::DeepseekChat),
                LlmModel::DeepSeek(DeepSeekModel::DeepseekReasoner),
                LlmModel::Gemini(GeminiModel::Gemini15Flash),
                LlmModel::Gemini(GeminiModel::Gemini15Flash8b),
                LlmModel::Gemini(GeminiModel::Gemini15Pro),
                LlmModel::Gemini(GeminiModel::Gemini20Flash),
                LlmModel::Gemini(GeminiModel::Gemini20FlashLite),
                LlmModel::Gemini(GeminiModel::Gemini25Flash),
                LlmModel::Gemini(GeminiModel::Gemini25FlashLite),
                LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview0617),
                LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview092025),
                LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0417),
                LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0520),
                LlmModel::Gemini(GeminiModel::Gemini25FlashPreview092025),
                LlmModel::Gemini(GeminiModel::Gemini25Pro),
                LlmModel::Gemini(GeminiModel::Gemini25ProPreview0506),
                LlmModel::Gemini(GeminiModel::Gemini25ProPreview0605),
                LlmModel::Gemini(GeminiModel::Gemini3FlashPreview),
                LlmModel::Gemini(GeminiModel::Gemini3ProPreview),
                LlmModel::Gemini(GeminiModel::GeminiLive25Flash),
                LlmModel::Gemini(GeminiModel::GeminiLive25FlashPreviewNativeAudio),
                LlmModel::Moonshot(MoonshotModel::KimiK20711Preview),
                LlmModel::Moonshot(MoonshotModel::KimiK20905Preview),
                LlmModel::Moonshot(MoonshotModel::KimiK2Thinking),
                LlmModel::Moonshot(MoonshotModel::KimiK2ThinkingTurbo),
                LlmModel::Moonshot(MoonshotModel::KimiK2TurboPreview),
                LlmModel::Moonshot(MoonshotModel::KimiK25),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude35Haiku),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaude37Sonnet),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeHaiku45),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus4),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus41),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus45),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus46),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet4),
                LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeSonnet45),
                LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityLargePreviewFree),
                LlmModel::OpenRouter(OpenRouterModel::ArceeAiTrinityMiniFree),
                LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30Mistral24b),
                LlmModel::OpenRouter(OpenRouterModel::CognitivecomputationsDolphin30R1Mistral24b),
                LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekChatV31),
                LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR10528Qwen38bFree),
                LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekR1Free),
                LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31Terminus),
                LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV31TerminusExacto),
                LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32),
                LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32Speciale),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20Flash001),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini20FlashExpFree),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Flash),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLite),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashLitePreview092025),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25FlashPreview092025),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Pro),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0506),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25ProPreview0605),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3FlashPreview),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemini3ProPreview),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bIt),
                LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bItFree),
                LlmModel::OpenRouter(OpenRouterModel::KwaipilotKatCoderProFree),
                LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama3370bInstructFree),
                LlmModel::OpenRouter(OpenRouterModel::MetaLlamaLlama4ScoutFree),
                LlmModel::OpenRouter(OpenRouterModel::MicrosoftMaiDsR1Free),
                LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimax01),
                LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM1),
                LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM2),
                LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM21),
                LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM25),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiCodestral2508),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512Free),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralMedium2507),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2505Free),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstralSmall2507),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiMistral7bInstructFree),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium3),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralMedium31),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralNemoFree),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3124bInstruct),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstruct),
                LlmModel::OpenRouter(OpenRouterModel::MistralaiMistralSmall3224bInstructFree),
                LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiDev72bFree),
                LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2),
                LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905),
                LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905Exacto),
                LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Thinking),
                LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK25),
                LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Free),
                LlmModel::OpenRouter(OpenRouterModel::NousresearchDeephermes3Llama38bPreview),
                LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes4405b),
                LlmModel::OpenRouter(OpenRouterModel::NousresearchHermes470b),
                LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotron3Nano30bA3bFree),
                LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano12bV2VlFree),
                LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2),
                LlmModel::OpenRouter(OpenRouterModel::NvidiaNemotronNano9bV2Free),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41Mini),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt4oMini),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Codex),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Image),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Mini),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Nano),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Pro),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Chat),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Codex),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMax),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMini),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Chat),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Codex),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Pro),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120b),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bExacto),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bFree),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20b),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20bFree),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOssSafeguard20b),
                LlmModel::OpenRouter(OpenRouterModel::OpenaiO4Mini),
                LlmModel::OpenRouter(OpenRouterModel::OpenrouterAuroraAlpha),
                LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockDashAlpha),
                LlmModel::OpenRouter(OpenRouterModel::OpenrouterSherlockThinkAlpha),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl7bInstructFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl32bInstructFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen25Vl72bInstructFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen314bFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725Free),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bThinking2507),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bInstruct2507),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bThinking2507),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen332bFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen34bFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen38bFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder30bA3bInstruct),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFlash),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderExacto),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Max),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstruct),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bInstructFree),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Next80bA3bThinking),
                LlmModel::OpenRouter(OpenRouterModel::QwenQwq32bFree),
                LlmModel::OpenRouter(OpenRouterModel::RekaaiRekaFlash3),
                LlmModel::OpenRouter(OpenRouterModel::SarvamaiSarvamMFree),
                LlmModel::OpenRouter(OpenRouterModel::StepfunStep35Flash),
                LlmModel::OpenRouter(OpenRouterModel::StepfunStep35FlashFree),
                LlmModel::OpenRouter(OpenRouterModel::ThudmGlmZ132bFree),
                LlmModel::OpenRouter(OpenRouterModel::TngtechTngR1tChimeraFree),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrok3),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Beta),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Mini),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrok3MiniBeta),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrok4),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrok4Fast),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrok41Fast),
                LlmModel::OpenRouter(OpenRouterModel::XAiGrokCodeFast1),
                LlmModel::OpenRouter(OpenRouterModel::XiaomiMimoV2Flash),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45Air),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45v),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46Exacto),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47Flash),
                LlmModel::OpenRouter(OpenRouterModel::ZAiGlm5),
                LlmModel::ZAi(ZAiModel::Glm45),
                LlmModel::ZAi(ZAiModel::Glm45Air),
                LlmModel::ZAi(ZAiModel::Glm45Flash),
                LlmModel::ZAi(ZAiModel::Glm45v),
                LlmModel::ZAi(ZAiModel::Glm46),
                LlmModel::ZAi(ZAiModel::Glm46v),
                LlmModel::ZAi(ZAiModel::Glm47),
                LlmModel::ZAi(ZAiModel::Glm47Flash),
                LlmModel::ZAi(ZAiModel::Glm5),
            ]
        });
        &ALL
    }
}

impl std::fmt::Display for LlmModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_name())
    }
}

impl std::str::FromStr for LlmModel {
    type Err = String;

    /// Parse a "provider:model" string into an `LlmModel`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (provider_str, model_str) = s.split_once(':').unwrap_or((s, ""));
        match provider_str {
            "anthropic" => match model_str {
                "claude-3-5-haiku-20241022" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::Claude35Haiku20241022))
                }
                "claude-3-5-sonnet-20240620" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20240620))
                }
                "claude-3-5-sonnet-20241022" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::Claude35Sonnet20241022))
                }
                "claude-3-7-sonnet-20250219" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::Claude37Sonnet20250219))
                }
                "claude-3-haiku-20240307" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::Claude3Haiku20240307))
                }
                "claude-3-opus-20240229" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::Claude3Opus20240229))
                }
                "claude-3-sonnet-20240229" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::Claude3Sonnet20240229))
                }
                "claude-haiku-4-5" => Ok(LlmModel::Anthropic(AnthropicModel::ClaudeHaiku45)),
                "claude-haiku-4-5-20251001" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::ClaudeHaiku4520251001))
                }
                "claude-opus-4-0" => Ok(LlmModel::Anthropic(AnthropicModel::ClaudeOpus40)),
                "claude-opus-4-1" => Ok(LlmModel::Anthropic(AnthropicModel::ClaudeOpus41)),
                "claude-opus-4-1-20250805" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::ClaudeOpus4120250805))
                }
                "claude-opus-4-20250514" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::ClaudeOpus420250514))
                }
                "claude-opus-4-5" => Ok(LlmModel::Anthropic(AnthropicModel::ClaudeOpus45)),
                "claude-opus-4-5-20251101" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::ClaudeOpus4520251101))
                }
                "claude-opus-4-6" => Ok(LlmModel::Anthropic(AnthropicModel::ClaudeOpus46)),
                "claude-sonnet-4-0" => Ok(LlmModel::Anthropic(AnthropicModel::ClaudeSonnet40)),
                "claude-sonnet-4-20250514" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::ClaudeSonnet420250514))
                }
                "claude-sonnet-4-5" => Ok(LlmModel::Anthropic(AnthropicModel::ClaudeSonnet45)),
                "claude-sonnet-4-5-20250929" => {
                    Ok(LlmModel::Anthropic(AnthropicModel::ClaudeSonnet4520250929))
                }
                _ => Err(format!("Unknown anthropic model: '{model_str}'")),
            },
            "deepseek" => match model_str {
                "deepseek-chat" => Ok(LlmModel::DeepSeek(DeepSeekModel::DeepseekChat)),
                "deepseek-reasoner" => Ok(LlmModel::DeepSeek(DeepSeekModel::DeepseekReasoner)),
                _ => Err(format!("Unknown deepseek model: '{model_str}'")),
            },
            "gemini" => match model_str {
                "gemini-1.5-flash" => Ok(LlmModel::Gemini(GeminiModel::Gemini15Flash)),
                "gemini-1.5-flash-8b" => Ok(LlmModel::Gemini(GeminiModel::Gemini15Flash8b)),
                "gemini-1.5-pro" => Ok(LlmModel::Gemini(GeminiModel::Gemini15Pro)),
                "gemini-2.0-flash" => Ok(LlmModel::Gemini(GeminiModel::Gemini20Flash)),
                "gemini-2.0-flash-lite" => Ok(LlmModel::Gemini(GeminiModel::Gemini20FlashLite)),
                "gemini-2.5-flash" => Ok(LlmModel::Gemini(GeminiModel::Gemini25Flash)),
                "gemini-2.5-flash-lite" => Ok(LlmModel::Gemini(GeminiModel::Gemini25FlashLite)),
                "gemini-2.5-flash-lite-preview-06-17" => {
                    Ok(LlmModel::Gemini(GeminiModel::Gemini25FlashLitePreview0617))
                }
                "gemini-2.5-flash-lite-preview-09-2025" => Ok(LlmModel::Gemini(
                    GeminiModel::Gemini25FlashLitePreview092025,
                )),
                "gemini-2.5-flash-preview-04-17" => {
                    Ok(LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0417))
                }
                "gemini-2.5-flash-preview-05-20" => {
                    Ok(LlmModel::Gemini(GeminiModel::Gemini25FlashPreview0520))
                }
                "gemini-2.5-flash-preview-09-2025" => {
                    Ok(LlmModel::Gemini(GeminiModel::Gemini25FlashPreview092025))
                }
                "gemini-2.5-pro" => Ok(LlmModel::Gemini(GeminiModel::Gemini25Pro)),
                "gemini-2.5-pro-preview-05-06" => {
                    Ok(LlmModel::Gemini(GeminiModel::Gemini25ProPreview0506))
                }
                "gemini-2.5-pro-preview-06-05" => {
                    Ok(LlmModel::Gemini(GeminiModel::Gemini25ProPreview0605))
                }
                "gemini-3-flash-preview" => Ok(LlmModel::Gemini(GeminiModel::Gemini3FlashPreview)),
                "gemini-3-pro-preview" => Ok(LlmModel::Gemini(GeminiModel::Gemini3ProPreview)),
                "gemini-live-2.5-flash" => Ok(LlmModel::Gemini(GeminiModel::GeminiLive25Flash)),
                "gemini-live-2.5-flash-preview-native-audio" => Ok(LlmModel::Gemini(
                    GeminiModel::GeminiLive25FlashPreviewNativeAudio,
                )),
                _ => Err(format!("Unknown gemini model: '{model_str}'")),
            },
            "moonshot" => match model_str {
                "kimi-k2-0711-preview" => Ok(LlmModel::Moonshot(MoonshotModel::KimiK20711Preview)),
                "kimi-k2-0905-preview" => Ok(LlmModel::Moonshot(MoonshotModel::KimiK20905Preview)),
                "kimi-k2-thinking" => Ok(LlmModel::Moonshot(MoonshotModel::KimiK2Thinking)),
                "kimi-k2-thinking-turbo" => {
                    Ok(LlmModel::Moonshot(MoonshotModel::KimiK2ThinkingTurbo))
                }
                "kimi-k2-turbo-preview" => {
                    Ok(LlmModel::Moonshot(MoonshotModel::KimiK2TurboPreview))
                }
                "kimi-k2.5" => Ok(LlmModel::Moonshot(MoonshotModel::KimiK25)),
                _ => Err(format!("Unknown moonshot model: '{model_str}'")),
            },
            "openrouter" => match model_str {
                "anthropic/claude-3.5-haiku" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::AnthropicClaude35Haiku,
                )),
                "anthropic/claude-3.7-sonnet" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::AnthropicClaude37Sonnet,
                )),
                "anthropic/claude-haiku-4.5" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::AnthropicClaudeHaiku45,
                )),
                "anthropic/claude-opus-4" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus4))
                }
                "anthropic/claude-opus-4.1" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus41))
                }
                "anthropic/claude-opus-4.5" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus45))
                }
                "anthropic/claude-opus-4.6" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::AnthropicClaudeOpus46))
                }
                "anthropic/claude-sonnet-4" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::AnthropicClaudeSonnet4,
                )),
                "anthropic/claude-sonnet-4.5" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::AnthropicClaudeSonnet45,
                )),
                "arcee-ai/trinity-large-preview:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::ArceeAiTrinityLargePreviewFree,
                )),
                "arcee-ai/trinity-mini:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::ArceeAiTrinityMiniFree,
                )),
                "cognitivecomputations/dolphin3.0-mistral-24b" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::CognitivecomputationsDolphin30Mistral24b,
                )),
                "cognitivecomputations/dolphin3.0-r1-mistral-24b" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::CognitivecomputationsDolphin30R1Mistral24b,
                )),
                "deepseek/deepseek-chat-v3.1" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::DeepseekDeepseekChatV31,
                )),
                "deepseek/deepseek-r1-0528-qwen3-8b:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::DeepseekDeepseekR10528Qwen38bFree,
                )),
                "deepseek/deepseek-r1:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::DeepseekDeepseekR1Free,
                )),
                "deepseek/deepseek-v3.1-terminus" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::DeepseekDeepseekV31Terminus,
                )),
                "deepseek/deepseek-v3.1-terminus:exacto" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::DeepseekDeepseekV31TerminusExacto,
                )),
                "deepseek/deepseek-v3.2" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::DeepseekDeepseekV32))
                }
                "deepseek/deepseek-v3.2-speciale" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::DeepseekDeepseekV32Speciale,
                )),
                "google/gemini-2.0-flash-001" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini20Flash001,
                )),
                "google/gemini-2.0-flash-exp:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini20FlashExpFree,
                )),
                "google/gemini-2.5-flash" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Flash))
                }
                "google/gemini-2.5-flash-lite" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini25FlashLite,
                )),
                "google/gemini-2.5-flash-lite-preview-09-2025" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini25FlashLitePreview092025,
                )),
                "google/gemini-2.5-flash-preview-09-2025" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini25FlashPreview092025,
                )),
                "google/gemini-2.5-pro" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::GoogleGemini25Pro))
                }
                "google/gemini-2.5-pro-preview-05-06" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini25ProPreview0506,
                )),
                "google/gemini-2.5-pro-preview-06-05" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini25ProPreview0605,
                )),
                "google/gemini-3-flash-preview" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini3FlashPreview,
                )),
                "google/gemini-3-pro-preview" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::GoogleGemini3ProPreview,
                )),
                "google/gemma-3-27b-it" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bIt))
                }
                "google/gemma-3-27b-it:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::GoogleGemma327bItFree))
                }
                "kwaipilot/kat-coder-pro:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::KwaipilotKatCoderProFree,
                )),
                "meta-llama/llama-3.3-70b-instruct:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MetaLlamaLlama3370bInstructFree,
                )),
                "meta-llama/llama-4-scout:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MetaLlamaLlama4ScoutFree,
                )),
                "microsoft/mai-ds-r1:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::MicrosoftMaiDsR1Free))
                }
                "minimax/minimax-01" => Ok(LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimax01)),
                "minimax/minimax-m1" => Ok(LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM1)),
                "minimax/minimax-m2" => Ok(LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM2)),
                "minimax/minimax-m2.1" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM21))
                }
                "minimax/minimax-m2.5" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::MinimaxMinimaxM25))
                }
                "mistralai/codestral-2508" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiCodestral2508,
                )),
                "mistralai/devstral-2512" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::MistralaiDevstral2512))
                }
                "mistralai/devstral-2512:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiDevstral2512Free,
                )),
                "mistralai/devstral-medium-2507" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiDevstralMedium2507,
                )),
                "mistralai/devstral-small-2505" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiDevstralSmall2505,
                )),
                "mistralai/devstral-small-2505:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiDevstralSmall2505Free,
                )),
                "mistralai/devstral-small-2507" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiDevstralSmall2507,
                )),
                "mistralai/mistral-7b-instruct:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiMistral7bInstructFree,
                )),
                "mistralai/mistral-medium-3" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiMistralMedium3,
                )),
                "mistralai/mistral-medium-3.1" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiMistralMedium31,
                )),
                "mistralai/mistral-nemo:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiMistralNemoFree,
                )),
                "mistralai/mistral-small-3.1-24b-instruct" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiMistralSmall3124bInstruct,
                )),
                "mistralai/mistral-small-3.2-24b-instruct" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiMistralSmall3224bInstruct,
                )),
                "mistralai/mistral-small-3.2-24b-instruct:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MistralaiMistralSmall3224bInstructFree,
                )),
                "moonshotai/kimi-dev-72b:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MoonshotaiKimiDev72bFree,
                )),
                "moonshotai/kimi-k2" => Ok(LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2)),
                "moonshotai/kimi-k2-0905" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK20905))
                }
                "moonshotai/kimi-k2-0905:exacto" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MoonshotaiKimiK20905Exacto,
                )),
                "moonshotai/kimi-k2-thinking" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::MoonshotaiKimiK2Thinking,
                )),
                "moonshotai/kimi-k2.5" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK25))
                }
                "moonshotai/kimi-k2:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::MoonshotaiKimiK2Free))
                }
                "nousresearch/deephermes-3-llama-3-8b-preview" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::NousresearchDeephermes3Llama38bPreview,
                )),
                "nousresearch/hermes-4-405b" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::NousresearchHermes4405b,
                )),
                "nousresearch/hermes-4-70b" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::NousresearchHermes470b,
                )),
                "nvidia/nemotron-3-nano-30b-a3b:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::NvidiaNemotron3Nano30bA3bFree,
                )),
                "nvidia/nemotron-nano-12b-v2-vl:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::NvidiaNemotronNano12bV2VlFree,
                )),
                "nvidia/nemotron-nano-9b-v2" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::NvidiaNemotronNano9bV2,
                )),
                "nvidia/nemotron-nano-9b-v2:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::NvidiaNemotronNano9bV2Free,
                )),
                "openai/gpt-4.1" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41)),
                "openai/gpt-4.1-mini" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt41Mini)),
                "openai/gpt-4o-mini" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt4oMini)),
                "openai/gpt-5" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5)),
                "openai/gpt-5-codex" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Codex)),
                "openai/gpt-5-image" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Image)),
                "openai/gpt-5-mini" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Mini)),
                "openai/gpt-5-nano" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Nano)),
                "openai/gpt-5-pro" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt5Pro)),
                "openai/gpt-5.1" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51)),
                "openai/gpt-5.1-chat" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Chat)),
                "openai/gpt-5.1-codex" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51Codex))
                }
                "openai/gpt-5.1-codex-max" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMax))
                }
                "openai/gpt-5.1-codex-mini" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt51CodexMini))
                }
                "openai/gpt-5.2" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52)),
                "openai/gpt-5.2-chat" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Chat)),
                "openai/gpt-5.2-codex" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Codex))
                }
                "openai/gpt-5.2-pro" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGpt52Pro)),
                "openai/gpt-oss-120b" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120b))
                }
                "openai/gpt-oss-120b:exacto" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::OpenaiGptOss120bExacto,
                )),
                "openai/gpt-oss-120b:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss120bFree))
                }
                "openai/gpt-oss-20b" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20b)),
                "openai/gpt-oss-20b:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiGptOss20bFree))
                }
                "openai/gpt-oss-safeguard-20b" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::OpenaiGptOssSafeguard20b,
                )),
                "openai/o4-mini" => Ok(LlmModel::OpenRouter(OpenRouterModel::OpenaiO4Mini)),
                "openrouter/aurora-alpha" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::OpenrouterAuroraAlpha))
                }
                "openrouter/sherlock-dash-alpha" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::OpenrouterSherlockDashAlpha,
                )),
                "openrouter/sherlock-think-alpha" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::OpenrouterSherlockThinkAlpha,
                )),
                "qwen/qwen-2.5-vl-7b-instruct:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen25Vl7bInstructFree,
                )),
                "qwen/qwen2.5-vl-32b-instruct:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen25Vl32bInstructFree,
                )),
                "qwen/qwen2.5-vl-72b-instruct:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen25Vl72bInstructFree,
                )),
                "qwen/qwen3-14b:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen314bFree))
                }
                "qwen/qwen3-235b-a22b-07-25" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22b0725))
                }
                "qwen/qwen3-235b-a22b-07-25:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen3235bA22b0725Free,
                )),
                "qwen/qwen3-235b-a22b-thinking-2507" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen3235bA22bThinking2507,
                )),
                "qwen/qwen3-235b-a22b:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen3235bA22bFree))
                }
                "qwen/qwen3-30b-a3b-instruct-2507" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen330bA3bInstruct2507,
                )),
                "qwen/qwen3-30b-a3b-thinking-2507" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen330bA3bThinking2507,
                )),
                "qwen/qwen3-30b-a3b:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen330bA3bFree))
                }
                "qwen/qwen3-32b:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen332bFree))
                }
                "qwen/qwen3-4b:free" => Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen34bFree)),
                "qwen/qwen3-8b:free" => Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen38bFree)),
                "qwen/qwen3-coder" => Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Coder)),
                "qwen/qwen3-coder-30b-a3b-instruct" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen3Coder30bA3bInstruct,
                )),
                "qwen/qwen3-coder-flash" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFlash))
                }
                "qwen/qwen3-coder:exacto" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderExacto))
                }
                "qwen/qwen3-coder:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen3CoderFree))
                }
                "qwen/qwen3-max" => Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwen3Max)),
                "qwen/qwen3-next-80b-a3b-instruct" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen3Next80bA3bInstruct,
                )),
                "qwen/qwen3-next-80b-a3b-instruct:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen3Next80bA3bInstructFree,
                )),
                "qwen/qwen3-next-80b-a3b-thinking" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::QwenQwen3Next80bA3bThinking,
                )),
                "qwen/qwq-32b:free" => Ok(LlmModel::OpenRouter(OpenRouterModel::QwenQwq32bFree)),
                "rekaai/reka-flash-3" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::RekaaiRekaFlash3))
                }
                "sarvamai/sarvam-m:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::SarvamaiSarvamMFree))
                }
                "stepfun/step-3.5-flash" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::StepfunStep35Flash))
                }
                "stepfun/step-3.5-flash:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::StepfunStep35FlashFree,
                )),
                "thudm/glm-z1-32b:free" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::ThudmGlmZ132bFree))
                }
                "tngtech/tng-r1t-chimera:free" => Ok(LlmModel::OpenRouter(
                    OpenRouterModel::TngtechTngR1tChimeraFree,
                )),
                "x-ai/grok-3" => Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrok3)),
                "x-ai/grok-3-beta" => Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Beta)),
                "x-ai/grok-3-mini" => Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrok3Mini)),
                "x-ai/grok-3-mini-beta" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrok3MiniBeta))
                }
                "x-ai/grok-4" => Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrok4)),
                "x-ai/grok-4-fast" => Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrok4Fast)),
                "x-ai/grok-4.1-fast" => Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrok41Fast)),
                "x-ai/grok-code-fast-1" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::XAiGrokCodeFast1))
                }
                "xiaomi/mimo-v2-flash" => {
                    Ok(LlmModel::OpenRouter(OpenRouterModel::XiaomiMimoV2Flash))
                }
                "z-ai/glm-4.5" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45)),
                "z-ai/glm-4.5-air" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45Air)),
                "z-ai/glm-4.5v" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm45v)),
                "z-ai/glm-4.6" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46)),
                "z-ai/glm-4.6:exacto" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm46Exacto)),
                "z-ai/glm-4.7" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47)),
                "z-ai/glm-4.7-flash" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm47Flash)),
                "z-ai/glm-5" => Ok(LlmModel::OpenRouter(OpenRouterModel::ZAiGlm5)),
                _ => Err(format!("Unknown openrouter model: '{model_str}'")),
            },
            "zai" => match model_str {
                "glm-4.5" => Ok(LlmModel::ZAi(ZAiModel::Glm45)),
                "glm-4.5-air" => Ok(LlmModel::ZAi(ZAiModel::Glm45Air)),
                "glm-4.5-flash" => Ok(LlmModel::ZAi(ZAiModel::Glm45Flash)),
                "glm-4.5v" => Ok(LlmModel::ZAi(ZAiModel::Glm45v)),
                "glm-4.6" => Ok(LlmModel::ZAi(ZAiModel::Glm46)),
                "glm-4.6v" => Ok(LlmModel::ZAi(ZAiModel::Glm46v)),
                "glm-4.7" => Ok(LlmModel::ZAi(ZAiModel::Glm47)),
                "glm-4.7-flash" => Ok(LlmModel::ZAi(ZAiModel::Glm47Flash)),
                "glm-5" => Ok(LlmModel::ZAi(ZAiModel::Glm5)),
                _ => Err(format!("Unknown zai model: '{model_str}'")),
            },
            "ollama" => Ok(LlmModel::Ollama(model_str.to_string())),
            "llamacpp" => Ok(LlmModel::LlamaCpp(model_str.to_string())),
            _ => Err(format!("Unknown provider: '{provider_str}'")),
        }
    }
}
