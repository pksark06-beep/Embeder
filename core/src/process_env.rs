// SPDX-License-Identifier: MPL-2.0
//! Keep known model credentials out of non-model child processes.

use std::process::Command;

const MODEL_SECRET_NAMES: &[&str] = &[
    "EMBEDER_LLM_API_KEY",
    "GEMINI_API_KEY",
    "GEMINI_AI_KEY",
    "GOOGLE_API_KEY",
    "AI_GATEWAY_API_KEY",
    "VERCEL_AI_GATEWAY_API_KEY",
    "OPENAI_API_KEY",
];

pub fn remove_model_secrets(command: &mut Command) {
    for name in MODEL_SECRET_NAMES {
        command.env_remove(name);
    }
}
