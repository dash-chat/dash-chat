Requirements for Dash Chat components
=====================================

- Requirements are in the form described in https://www.rfc-editor.org/rfc/rfc2119.
- This list of requirements is scoped to our MVP. 
- Intentions have less clarity than requirements. Whenever more clarity is achieved, they can be turned into requirements. They can also be read as TODOs for things to look into and refine.
- This is a living document. It should be always correct and never complete.
- We should have a high confidence in all of the requirements; changing a requirement is a big deal.


Dash Chat App
=============

Usability
---------

### Requirements

- The app MUST support mobile and desktop platforms.
- The app MUST introduce itself in a way to set the right expectations for the user to understand how to use it.
- The app MUST support different languages
- When switching between and operating in different network environments, the app MUST provide enough information for the user to have expectations around functional limitations for that situation.
    - example: can't access a relay server, your message will only be sent if recipients are online when you are
    - example: you're on a Bluetooth mesh only, delays may be much higher and delivery is not guaranteed (and MUCH higher for a LoRa mesh)
- The app SHOULD be built as a library so that it’s easily embeddable in other applications.
- Mobile users SHOULD receive notifications with a delay comparable to Signal.


### Intentions

- With internet, the UX **fits** the expectations that users have from other direct and group messaging chat applications as much as possible (i.e. Signal).
- Without internet, voice messages, images, and videos should be transmitted as well as possible and in as many situations as possible.
- In low-bandwidth situations, we may consider speech-to-text + text-to-speech to provide voice messages while only transmitting text
- personal names/if someone takes my icon/takes my name - harder to spoof
- rough: We want higher bandwidth protocols like BLE to not have to make big sacrifices to support LoRa (we'll figure out the unification later)


Reliability
-----------

### Requirements

- With internet, users MUST accomplish with the app everything that they've come to rely on being able to accomplish with Signal.
- The app MUST work on everyday devices, and on older versions of Android and iOS.
- The app MUST NOT rely on the internet as its only medium of communication between users.
- In intranet circumstances, the app MUST work as well as possible and in as many situations as possible.
- Without internet, the app MUST support relaying of messages through third party peers.
- The app MUST support WiFi LANs as a local transport.
- The app MUST support BLE as a local transport.
- With internet, the app SHOULD NOT drain battery life unnecessarily.
- The app MAY support LoRa as a mesh transport.


### Intentions

- The app should work as well as possible when offline, while also setting realistic expectations about the delivery and receipt of messages.
- The app should support as many users as possible, with or without internet.
- Without internet, the app should work with as many local networking situations as possible. Variables to consider:
    - Transports available on users' devices, and general system capabilities.
    - Size of the network.
    - Density of the network.


Security
--------


### Requirements

- All actions taken by users MUST BE authenticated and verifiable without the need of a centralized authority.
- The content of the messages MUST only be readable by their intended recipients, if their devices are not compromised.
- All data in the app on the users' devices MUST be encrypted at rest.
- Users' PII (personally identifiable information) MUST NOT be necessary for any system functionality.
- Parties who are not authorized to do so MUST NOT be able to remotely trigger processes in users' devices. Remote processes able to be triggered with authorization include:
    - Starting the app to receive a push notification.
    - Deleting data.
- The app SHOULD work with any VPN that the user chooses to use. 
- App initialization, account creation, contact adding, and messaging MUST NOT rely on the internet.
- Any internet based infrastructure SHOULD NOT keep any metadata more than it is necessary.
- Any third party relays SHOULD NOT keep any metadata more than is necessary.


### Intentions

- The app SHOULD expose as little metadata as possible that could be used to infer users' personal or behaviour information without compromising usability and reliability (at security level 1).
- Internet infrastructure servers SHOULD be as secure as possible from DoS attacks.
- Internet infrastructure servers SHOULD be as secure as possible from SPAM attacks.
- Users SHOULD NOT be able to be impersonated by an attacker that changes their profile information.
    - TODO: turn into a requirement when there is more clarity around the mitigation mechanisms.


"Servers"
=========

We use "Servers" to refer to any device meant to facilitate storage and exchange of messages for multiple people.

Current instances of Servers include the Cloud Server, which is publicly accessible and designed to handle high loads, and the Local Message Hubs, which are hosted primarly on LANs and may run on modest hardware.

Usability
---------

### Requirements
- Cloud Servers MUST provide an experience comprable to Whatsapp/Signal
- Local Message Hubs MUST be able to sync with each other
- Local Message Hubs MUST be able to load balance when on the same network
- All Servers SHOULD delete message data once delivered to their final location

### Intentions

- Users SHOULD experience syncing 
- Message hubs MUST be able to relay big files, like video, images and audio.
- Cloud servers SHOULD be able to store backups of users' chats if they opt in  (like signal/whatsapp)


Reliability
-----------

### Requirements
- MUST be resilient, reduntant, and reliable

### Intentions

- MUST be resistant to DoS and spam attacks
- MUST be able to enforce rate limits for existing groups
- SHOULD prioritize serving existing groups over creating new groups if resources are limited

Security
--------

### Requirements

- IT WOULD BE NICE IF the operator cannot map the social graph of user pubkeys
- If compromised, attacker MUST NOT be able to map the social graph of user pubkeys

Cloud Servers
===========

The Cloud Server is maintained by Dash Chat, targets 100% uptime, and uses a traditional distributed database.

Everything required by Message Hubs in general, plus the following:


Usability
---------
### Requirements
### Intentions

Reliability
-----------
### Requirements
### Intentions

Security
--------
### Requirements
### Intentions



Local Message Hub
===========

A version of Message Hubs designed to run within a LAN. In contrast to "internet" mailboxes, Local Message Hubs can be run by anyone, have no uptime guarantees, and are designed exclusively for horizontal scaling.

Everything general to Message Hubs, plus the following:

Usability
---------
### Requirements
### Intentions

- Server SHOULD be easily discoverable and usable by nodes on the local network 


Reliability
-----------
### Requirements
### Intentions


Security
--------
### Requirements
### Intentions




