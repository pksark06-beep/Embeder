# SPDX-License-Identifier: MPL-2.0
"""Offline checks for model endpoint validation."""

import unittest
from unittest.mock import patch

from codegen_server import _call_gemini, _validated_base_url


class ModelEndpointTests(unittest.TestCase):
    def test_https_and_loopback_are_allowed(self):
        self.assertEqual(_validated_base_url("https://api.openai.com/v1/"), "https://api.openai.com/v1")
        self.assertEqual(_validated_base_url("http://127.0.0.1:11434/v1"), "http://127.0.0.1:11434/v1")

    def test_remote_plain_http_and_embedded_credentials_are_rejected(self):
        for value in ("http://example.com/v1", "https://user:secret@example.com/v1",
                      "https://example.com/v1?token=secret", "file:///tmp/model"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                _validated_base_url(value)

    def test_gemini_key_is_sent_in_header_not_url(self):
        cfg = {"base_url": "https://generativelanguage.googleapis.com/v1beta",
               "model": "test-model", "api_key": "test-secret"}
        with patch("codegen_server._http_post_json", return_value={"candidates": [
            {"content": {"parts": [{"text": "ok"}]}}
        ]}) as post:
            self.assertEqual(_call_gemini(cfg, "system", "user", 0.2, 256, 5), "ok")
        url, headers, _, _ = post.call_args.args
        self.assertNotIn("test-secret", url)
        self.assertEqual(headers["x-goog-api-key"], "test-secret")


if __name__ == "__main__":
    unittest.main()
