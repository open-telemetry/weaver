// Examples of comments for group device

  /**
   * A unique identifier representing the device
   * <p>
   * The device identifier MUST only be defined using the values outlined below. This value is not an advertising identifier and MUST NOT be used as such. On iOS (Swift or Objective-C), this value MUST be equal to the <a href="https://developer.apple.com/documentation/uikit/uidevice/1620059-identifierforvendor">vendor identifier</a>. On Android (Java or Kotlin), this value MUST be equal to the Firebase Installation ID or a globally unique UUID which is persisted across sessions in your application. More information can be found <a href="https://developer.android.com/training/articles/user-data-ids">here</a> on best practices and exact implementation details. Caution should be taken when storing personal data or anything which can identify a user. GDPR and data protection laws may apply, ensure you do your own due diligence
   */
  static DEVICE_ID = "";

  /**
   * The name of the device manufacturer
   * <p>
   * The Android OS provides this field via <a href="https://developer.android.com/reference/android/os/Build#MANUFACTURER">Build</a>. iOS apps SHOULD hardcode the value {@code Apple}
   */
  static DEVICE_MANUFACTURER = "";

  /**
   * The model identifier for the device
   * <p>
   * It's recommended this value represents a machine-readable version of the model identifier rather than the market or consumer-friendly name of the device
   */
  static DEVICE_MODEL_IDENTIFIER = "";

  /**
   * The marketing name for the device model
   * <p>
   * It's recommended this value represents a human-readable version of the device model rather than a machine-readable alternative
   */
  static DEVICE_MODEL_NAME = "";


// Examples of comments for group dns

  /**
   * The name being queried.
   * <p>
   * If the name field contains non-printable characters (below 32 or above 126), those characters should be represented as escaped base 10 integers (\DDD). Back slashes and quotes should be escaped. Tabs, carriage returns, and line feeds should be converted to \t, \r, and \n respectively
   */
  static DNS_QUESTION_NAME = "";


// Examples of comments for group error

  /**
   * Describes a class of error the operation ended with.
   * <p>
   * The {@code error.type} SHOULD be predictable, and SHOULD have low cardinality.
   * <p>
   * When {@code error.type} is set to a type (e.g., an exception type), its
   * canonical class name identifying the type within the artifact SHOULD be used.
   * <p>
   * Instrumentations SHOULD document the list of errors they report.
   * <p>
   * The cardinality of {@code error.type} within one instrumentation library SHOULD be low.
   * Telemetry consumers that aggregate data from multiple instrumentation libraries and applications
   * should be prepared for {@code error.type} to have high cardinality at query time when no
   * additional filters are applied.
   * <p>
   * If the operation has completed successfully, instrumentations SHOULD NOT set {@code error.type}.
   * <p>
   * If a specific domain defines its own set of error identifiers (such as HTTP or gRPC status codes),
   * it's RECOMMENDED to:
   * <ul>
   *   <li>Use a domain-specific attribute
   *   <li>Set {@code error.type} to capture all errors, regardless of whether they are defined within the domain-specific set or not
   * </ul>
   * <p>
   * And something more
   */
  static ERROR_TYPE = "";


// Examples of comments for group other

  /**
   * This is a brief description of the attribute + a short link <a href="https://www.opentelemetry.com">OTEL</a>.
   * <p>
   * This is a note about the attribute {@code attr}. It can be multiline.
   * <p>
   * It can contain a list:
   * <ul>
   *   <li>item <strong>1</strong>,
   *   <li>lorem ipsum dolor sit amet, consectetur
   * adipiscing elit sed do eiusmod tempor
   * <a href="https://www.loremipsum.com">incididunt</a> ut labore et dolore magna aliqua.
   *   <li>item 2
   *   <li>lorem ipsum dolor sit amet, consectetur
   * adipiscing elit sed do eiusmod tempor
   * incididunt ut labore et dolore magna aliqua.
   * </ul>
   * <p>
   * And an <strong>inline code snippet</strong>: {@code Attr.attr}.
   * <h1>Summary</h1>
   * <h2>Examples</h2>
   * <ol>
   *   <li>Example 1
   *   <li><a href="https://loremipsum.com">Example</a> with lorem ipsum dolor sit amet, consectetur adipiscing elit
   * <a href="https://loremipsum.com">sed</a> do eiusmod tempor incididunt ut
   * <a href="https://loremipsum.com">labore</a> et dolore magna aliqua.
   *   <li>Example 3
   * </ol>
   * <h2>Appendix</h2>
   * <ul>
   *   <li><a href="https://www.link1.com">Link 1</a>
   *   <li><a href="https://www.link2.com">Link 2</a>
   *   <li>A very long item in the list with lorem ipsum dolor sit amet, consectetur adipiscing elit sed do eiusmod
   * tempor incididunt ut labore et dolore magna aliqua.
   * </ul>
   * <blockquote>
   * This is a blockquote.
   * It can contain multiple lines.
   * Lorem ipsum dolor sit amet, consectetur adipiscing
   * elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</blockquote>
   * <blockquote>
   * [!NOTE] Something very important here</blockquote>
   */
  static ATTR = "";


