  DEVICE_ID: Final = ""
  """
  A unique identifier representing the device
  The device identifier MUST only be defined using the values outlined below. This value is not an advertising identifier and MUST NOT be used as such. On iOS (Swift or Objective-C), this value MUST be equal to the [vendor identifier](https://developer.apple.com/documentation/uikit/uidevice/1620059-identifierforvendor). On Android (Java or Kotlin), this value MUST be equal to the Firebase Installation ID or a globally unique UUID which is persisted across sessions in your application. More information can be found [here](https://developer.android.com/training/articles/user-data-ids) on best practices and exact implementation details. Caution should be taken when storing personal data or anything which can identify a user. GDPR and data protection laws may apply, ensure you do your own due diligence
  """

  DEVICE_MANUFACTURER: Final = ""
  """
  The name of the device manufacturer
  The Android OS provides this field via [Build](https://developer.android.com/reference/android/os/Build#MANUFACTURER). iOS apps SHOULD hardcode the value `Apple`
  """

  DEVICE_MODEL_IDENTIFIER: Final = ""
  """
  The model identifier for the device
  It's recommended this value represents a machine-readable version of the model identifier rather than the market or consumer-friendly name of the device
  """

  DEVICE_MODEL_NAME: Final = ""
  """
  The marketing name for the device model
  It's recommended this value represents a human-readable version of the device model rather than a machine-readable alternative
  """

  DNS_QUESTION_NAME: Final = ""
  """
  The name being queried.
  If the name field contains non-printable characters (below 32 or above 126), those characters should be represented as escaped base 10 integers (\\DDD). Back slashes and quotes should be escaped. Tabs, carriage returns, and line feeds should be converted to \\t, \\r, and \\n respectively
  """

  ERROR_TYPE: Final = ""
  """
  Describes a class of error the operation ended with.
  The `error.type` SHOULD be predictable, and SHOULD have low cardinality.
  When `error.type` is set to a type (e.g., an exception type), its
  canonical class name identifying the type within the artifact SHOULD be used.
  Instrumentations SHOULD document the list of errors they report.
  The cardinality of `error.type` within one instrumentation library SHOULD be low.
  Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
  should be prepared for `error.type` to have high cardinality at query time when no
  additional filters are applied.
  If the operation has completed successfully, instrumentations SHOULD NOT set `error.type`.
  If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
  it's RECOMMENDED to:
  
  - Use a domain-specific attribute
  - Set `error.type` to capture all errors, regardless of whether they are defined within the domain-specific set or not
  """

  ATTR: Final = ""
  """
  This is a brief description of the attribute + a short link [OTEL](https://www.opentelemetry.com).
  This is a note about the attribute `attr`. It can be multiline.
  It can contain a list:
  
  - item **1**,
  - item 2.
  And an **inline code snippet**: `Attr.attr`.
  
  # Summary
  ## Examples:
  1. Example 1
  2. Example 2
  3. Example 3
  
  ## Appendix
  - [Link 1](https://www.link1.com)
  - [Link 2](https://www.link2.com)
  - A very long item in the list with lorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod
  tempor incididunt ut labore et dolore magna aliqua.
  
  > This is a blockquote.
  It can contain multiple lines.
  
  > [!NOTE] Something very important here
  """

