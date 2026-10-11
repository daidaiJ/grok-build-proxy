//! 成本计算：token 计数 × 目录价 → USD（纯函数，渲染层只管格式化）。
//!
//! 账本语义（`xai-chat-state::usage` / `model_usage_ledger`）：
//! - OpenAI 兼容网关（常态）：`cached_read ⊆ prompt`（`prompt_tokens_details.cached_tokens`），
//!   计费输入 = `prompt − cached_read`，缓存读按 cache_read 价另计；
//! - Anthropic 风格 usage 才有 `cache_creation`（OpenAI 兼容恒 0），此时 `prompt`
//!   已排除缓存读，不减。

use crate::types::ModelPricing;

/// 一个模型的窗口 token 计数（与 [`crate::types::ModelPricing`] 同源同窗）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageTokens {
    pub prompt: u64,
    pub completion: u64,
    pub cached_read: u64,
    pub cache_creation: u64,
}

/// 计算一次窗口成本（USD）。任一「token 数 > 0 的成分」缺价 → `None`
/// （整条按无价处理，渲染层显示 `–` 并从合计剔除）；全零用量 → `Some(0.0)`。
pub fn cost_usd(pricing: &ModelPricing, usage: &UsageTokens) -> Option<f64> {
    // cache_creation > 0 仅出现在 Anthropic 风格 usage 上（该口径 prompt 已排除缓存读）。
    let billed_input = if usage.cache_creation > 0 {
        usage.prompt
    } else {
        usage.prompt.saturating_sub(usage.cached_read)
    };
    let mut cost = 0.0;
    if billed_input > 0 {
        cost += billed_input as f64 / 1_000_000.0 * pricing.input?;
    }
    if usage.completion > 0 {
        cost += usage.completion as f64 / 1_000_000.0 * pricing.output?;
    }
    if usage.cached_read > 0 {
        cost += usage.cached_read as f64 / 1_000_000.0 * pricing.cache_read?;
    }
    if usage.cache_creation > 0 {
        cost += usage.cache_creation as f64 / 1_000_000.0 * pricing.cache_write?;
    }
    Some(cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pricing(
        input: Option<f64>,
        output: Option<f64>,
        cache_read: Option<f64>,
        cache_write: Option<f64>,
    ) -> ModelPricing {
        ModelPricing {
            input,
            output,
            cache_read,
            cache_write,
        }
    }

    #[test]
    fn openai_style_cached_read_is_a_subset_of_prompt() {
        let p = pricing(Some(3.0), Some(15.0), Some(0.3), None);
        let u = UsageTokens {
            prompt: 1_000_000,
            completion: 100_000,
            cached_read: 400_000,
            cache_creation: 0,
        };
        // 计费输入 = 1M − 0.4M = 0.6M × $3 = 1.8；输出 0.1M × 15 = 1.5；缓存读 0.4M × 0.3 = 0.12
        let cost = cost_usd(&p, &u).expect("priced");
        assert!((cost - 3.42).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn anthropic_style_prompt_already_excludes_cached_read() {
        let p = pricing(Some(3.0), Some(15.0), Some(0.3), Some(3.75));
        let u = UsageTokens {
            prompt: 500_000,
            completion: 0,
            cached_read: 200_000,
            cache_creation: 50_000,
        };
        // prompt 0.5M × 3 = 1.5；缓存读 0.2M × 0.3 = 0.06；缓存写 0.05M × 3.75 = 0.1875
        let cost = cost_usd(&p, &u).expect("priced");
        assert!((cost - 1.7475).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn zero_usage_costs_zero_even_without_prices() {
        let p = pricing(None, None, None, None);
        assert_eq!(cost_usd(&p, &UsageTokens::default()), Some(0.0));
    }

    #[test]
    fn missing_price_for_nonzero_component_is_unpriced() {
        let u = UsageTokens {
            prompt: 1_000,
            completion: 1_000,
            cached_read: 500,
            cache_creation: 0,
        };
        // 缓存读有量但无价 → 整条无价（宁可显示 – 也不给低估的数）。
        let p = pricing(Some(3.0), Some(15.0), None, None);
        assert_eq!(cost_usd(&p, &u), None);
        // 同一模型无缓存读时则有价。
        let u_no_cache = UsageTokens { cached_read: 0, ..u };
        assert!(cost_usd(&p, &u_no_cache).is_some());
    }

    #[test]
    fn fully_unpriced_model_is_none_when_used() {
        let p = pricing(None, None, None, None);
        let u = UsageTokens {
            prompt: 10,
            completion: 10,
            cached_read: 0,
            cache_creation: 0,
        };
        assert_eq!(cost_usd(&p, &u), None);
    }

    #[test]
    fn saturated_subtraction_never_panics_on_bad_data() {
        let p = pricing(Some(1.0), Some(1.0), Some(0.0), None);
        let u = UsageTokens {
            prompt: 100,
            completion: 0,
            cached_read: 500,
            cache_creation: 0,
        };
        assert_eq!(cost_usd(&p, &u), Some(0.0));
    }
}
