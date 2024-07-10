## Events Namespace `gen_ai.content`


## Event `gen_ai.content.completion`

Note: 
Brief: In the lifetime of an GenAI span, events for prompts sent and completions received may be created, depending on the configuration of the instrumentation.

Requirement level: 
Stability: 

### Body Fields

No event body defined.### Attributes


#### Attribute `gen_ai.completion`

The full response received from the GenAI model.


It's RECOMMENDED to format completions as JSON string matching [OpenAI messages format](https://platform.openai.com/docs/guides/text-generation)

- Requirement Level: Conditionally Required - if and only if corresponding event is enabled
  
- Type: string
- Examples: [
    "[{'role': 'assistant', 'content': 'The capital of France is Paris.'}]",
]
  
- Stability: Experimental
  
  
  
## Event `gen_ai.content.prompt`

Note: 
Brief: In the lifetime of an GenAI span, events for prompts sent and completions received may be created, depending on the configuration of the instrumentation.

Requirement level: 
Stability: 

### Body Fields

No event body defined.### Attributes


#### Attribute `gen_ai.prompt`

The full prompt sent to the GenAI model.


It's RECOMMENDED to format prompts as JSON string matching [OpenAI messages format](https://platform.openai.com/docs/guides/text-generation)

- Requirement Level: Conditionally Required - if and only if corresponding event is enabled
  
- Type: string
- Examples: [
    "[{'role': 'user', 'content': 'What is the capital of France?'}]",
]
  
- Stability: Experimental
  
  
  