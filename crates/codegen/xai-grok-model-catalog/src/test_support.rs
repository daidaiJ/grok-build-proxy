//! 测试夹具：modelq 同款响应形态的最小 JSON（真实字段、真实量级）。

/// OpenRouter `/api/v1/models` 片段：grok-4 全价 / deepseek 免费+付费对 / 无价条目 /
/// zai glm（vendor 别名与主源平分测试用）。
pub fn openrouter_fixture() -> &'static str {
    r#"{
  "data": [
    {
      "id": "x-ai/grok-4",
      "canonical_slug": "x-ai/grok-4",
      "name": "Grok 4",
      "context_length": 256000,
      "supported_parameters": ["reasoning", "temperature"],
      "architecture": { "input_modalities": ["text", "image"] },
      "pricing": {
        "prompt": "0.000003",
        "completion": "0.000015",
        "input_cache_read": "0.0000003",
        "input_cache_write": "-1",
        "request": "0",
        "image": "0"
      },
      "top_provider": { "context_length": 256000, "max_completion_tokens": 32000 }
    },
    {
      "id": "deepseek/deepseek-chat-v3.1",
      "canonical_slug": "deepseek/deepseek-chat-v3.1",
      "name": "DeepSeek V3.1",
      "context_length": 163840,
      "supported_parameters": ["temperature"],
      "pricing": {
        "prompt": "0.0000002",
        "completion": "0.0000008",
        "input_cache_read": "0.00000005",
        "input_cache_write": "0"
      },
      "top_provider": { "context_length": 163840, "max_completion_tokens": 163840 }
    },
    {
      "id": "deepseek/deepseek-chat-v3.1:free",
      "canonical_slug": "deepseek/deepseek-chat-v3.1",
      "name": "DeepSeek V3.1 (free)",
      "context_length": 163840,
      "pricing": {
        "prompt": "0",
        "completion": "0",
        "input_cache_read": "0",
        "input_cache_write": "0"
      },
      "top_provider": { "context_length": 163840, "max_completion_tokens": 0 }
    },
    {
      "id": "vendor/unpriced",
      "name": "Unpriced",
      "context_length": 0,
      "pricing": { "prompt": "", "completion": "-1", "input_cache_read": "", "input_cache_write": "" },
      "top_provider": { "context_length": 0, "max_completion_tokens": 0 }
    },
    {
      "id": "zai/glm-5.3-flash",
      "name": "GLM-5.3-Flash",
      "context_length": 128000,
      "supported_parameters": ["reasoning", "reasoning_effort", "temperature"],
      "reasoning": { "mandatory": true, "default_enabled": true, "supported_efforts": ["max", "high", "low"], "default_effort": "max" },
      "pricing": {
        "prompt": "0.00000015",
        "completion": "0.0000025",
        "input_cache_read": "0.00000002",
        "input_cache_write": "0"
      },
      "top_provider": { "context_length": 128000, "max_completion_tokens": 96000 }
    }
  ]
}"#
}

/// models.dev `/api.json` 片段：zai（vendor 别名折叠目标）/ anthropic（canonical）/
/// bare-provider（全缺省字段）。
pub fn models_dev_fixture() -> &'static str {
    r#"{
  "zai": {
    "id": "zai",
    "name": "Z.ai",
    "models": {
      "glm-5.3-flash": {
        "name": "GLM-5.3-Flash",
        "reasoning": true,
        "reasoning_options": [{ "type": "toggle" }, { "type": "effort", "values": ["low", "high", "max"] }],
        "modalities": { "input": ["text"] },
        "limit": { "context": 128000, "output": 96000 },
        "cost": { "input": 0.15, "output": 2.5, "cache_read": 0.02 }
      }
    }
  },
  "anthropic": {
    "id": "anthropic",
    "name": "Anthropic",
    "models": {
      "claude-sonnet-4-5": {
        "name": "Claude Sonnet 4.5",
        "canonical_model_id": "anthropic/claude-sonnet-4-5",
        "reasoning": false,
        "limit": { "context": 200000, "output": 64000 },
        "cost": { "input": 3.0, "output": 15.0, "cache_read": 0.3, "cache_write": 3.75 }
      }
    }
  },
  "deepseek": {
    "id": "deepseek",
    "name": "DeepSeek",
    "models": {
      "bare-model": { "name": "Bare Model" }
    }
  }
}"#
}

/// 测试目录：双源夹具合并（真实运行是单源生效，这里只为覆盖匹配档位）。
/// 经 [`ModelCatalog::new`] 重建索引（直接 extend 会绕过索引不变式）。
pub fn test_catalog() -> crate::types::ModelCatalog {
    let mut cat = crate::openrouter::parse(openrouter_fixture()).expect("openrouter fixture");
    let dev = crate::modelsdev::parse(models_dev_fixture()).expect("models.dev fixture");
    cat.entries.extend(dev.entries);
    crate::types::ModelCatalog::new(cat.entries)
}
