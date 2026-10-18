# Tags

```rain
let Cat = internal._tag()
let Dog = internal._tag()

// A closed enum of pets
let Pet = internal._closed([Cat, Dog])

// An open enum of transport options
let Transport = internal._open()
let Car = internal._tag([Transport])
let Train = internal._tag([Transport])

// Custom Types
let StringLiteral = internal._tag([TypeUnniverse])
let String = internal._tag([TypeUnniverse])
let Integer = internal._tag([])
```
