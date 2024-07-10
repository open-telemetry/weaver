## Namespace `gen_ai`

### Attributes


#### Attribute `gen_ai.completion`

The full response received from the GenAI model.


It's RECOMMENDED to format completions as JSON string matching [OpenAI messages format](https://platform.openai.com/docs/guides/text-generation)

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "[{'role': 'assistant', 'content': 'The capital of France is Paris.'}]",
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.operation.name`

The name of the operation being performed.


If one of the predefined values applies, but specific system uses a different name it's RECOMMENDED to document it in the semantic conventions for specific GenAI system and use system-specific name in the instrumentation. If a different name is not documented, instrumentation libraries SHOULD use applicable predefined value.

- Requirement Level: Recommended
  
- Type: Enum [chat, text_completion]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.prompt`

The full prompt sent to the GenAI model.


It's RECOMMENDED to format prompts as JSON string matching [OpenAI messages format](https://platform.openai.com/docs/guides/text-generation)

- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "[{'role': 'user', 'content': 'What is the capital of France?'}]",
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.frequency_penalty`

The frequency penalty setting for the GenAI request.


- Requirement Level: Recommended
  
- Type: double
- Examples: [
    0.1,
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.max_tokens`

The maximum number of tokens the model generates for a request.


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    100,
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.model`

The name of the GenAI model a request is being made to.


- Requirement Level: Recommended
  
- Type: string
- Examples: gpt-4
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.presence_penalty`

The presence penalty setting for the GenAI request.


- Requirement Level: Recommended
  
- Type: double
- Examples: [
    0.1,
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.stop_sequences`

List of sequences that the model will use to stop generating further tokens.


- Requirement Level: Recommended
  
- Type: string[]
- Examples: [
    "forest",
    "lived",
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.temperature`

The temperature setting for the GenAI request.


- Requirement Level: Recommended
  
- Type: double
- Examples: [
    0.0,
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.top_k`

The top_k sampling setting for the GenAI request.


- Requirement Level: Recommended
  
- Type: double
- Examples: [
    1.0,
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.request.top_p`

The top_p sampling setting for the GenAI request.


- Requirement Level: Recommended
  
- Tag: llm-generic-request
  
- Type: double
- Examples: [
    1.0,
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.response.finish_reasons`

Array of reasons the model stopped generating tokens, corresponding to each generation received.


- Requirement Level: Recommended
  
- Type: string[]
- Examples: [
    "stop",
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.response.id`

The unique identifier for the completion.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "chatcmpl-123",
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.response.model`

The name of the model that generated the response.


- Requirement Level: Recommended
  
- Type: string
- Examples: [
    "gpt-4-0613",
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.system`

The Generative AI product as identified by the client or server instrumentation.


The `gen_ai.system` describes a family of GenAI models with specific model identified
by `gen_ai.request.model` and `gen_ai.response.model` attributes.

The actual GenAI product may differ from the one identified by the client.
For example, when using OpenAI client libraries to communicate with Mistral, the `gen_ai.system`
is set to `openai` based on the instrumentation's best knowledge.

For custom model, a custom friendly name SHOULD be used.
If none of these options apply, the `gen_ai.system` SHOULD be set to `_OTHER`.

- Requirement Level: Recommended
  
- Type: Enum [openai, vertex_ai, anthropic, cohere]
- Examples: openai
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.token.type`

The type of token being counted.


- Requirement Level: Recommended
  
- Type: Enum [input, output]
- Examples: [
    "input",
    "output",
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.usage.input_tokens`

The number of tokens used in the GenAI input (prompt).


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    100,
]
  
- Stability: Experimental
  
  
#### Attribute `gen_ai.usage.output_tokens`

The number of tokens used in the GenAI response (completion).


- Requirement Level: Recommended
  
- Type: int
- Examples: [
    180,
]
  
- Stability: Experimental
  
  