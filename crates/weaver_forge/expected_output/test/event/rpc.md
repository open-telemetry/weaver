## Events Namespace `rpc`


## Event `rpc.message`

Note: 
Brief: RPC received/sent message.
Requirement level: 
Stability: 

### Body Fields

No event body defined.### Attributes


#### Attribute `rpc.message.type`

Whether this is a received or sent message.


- Requirement Level: Recommended
  
- Type: Enum [SENT, RECEIVED]
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.id`

MUST be calculated as two different counters starting from `1` one for sent messages and one for received message.


This way we guarantee that the values will be consistent between different implementations.

- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.compressed_size`

Compressed size of the message in bytes.


- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
#### Attribute `rpc.message.uncompressed_size`

Uncompressed size of the message in bytes.


- Requirement Level: Recommended
  
- Type: int
  
- Stability: Experimental
  
  
  