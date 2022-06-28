## Welcome to Star Lane

Starlane is a messaging framework built to faciliate realtime communication between the various components of a distributed application.

### How does Starlane Differ from other Message Brokers?

Traditional message brokers such as Kafka and RabbitMQ are primarly used to decouple an event trigger from a task.  This is done by placing a message in a queue to be processed later by a worker when resources allow. Starlane is not a replacement for these solutions.  

Starlane allows an application developer to pin an address to a specific component within a microservice in order to send and receive realtime messages.  The component address can also broadcast messsages to any addresses that are "Watching".


