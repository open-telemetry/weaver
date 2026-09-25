# SPDX-License-Identifier: Apache-2.0

def public_attribute_names:
  semconv_attributes
  | map(.name)
  | map(select(startswith("public.")))
  | sort;

def should_generate: $params.generate_public_attributes;
