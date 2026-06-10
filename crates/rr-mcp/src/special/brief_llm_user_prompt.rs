pub(crate) static GENERATED_BRIEF_USER_PROMPT_TEMPLATE: &str = "\
  Return only a JSON object matching this schema:
  {\"summary\":\"string\",\"five_w\":{\"who\":\"string\",\"what\":\"string\",\"when\":\"string\",\"where\":\"string\",\"why\":\"string\",\"how\":\"string\"},\"unknowns\":[\"string\"]}

  Use only this evidence packet. Unsupported facts must go in unknowns.

  Evidence packet:
  {evidence_json}";
