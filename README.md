# raptorCDN

The overall goal is to provide a somewhat-decentralized P2P CDN based on raptor codes. At the minimum, be able to recover quickly from the network if the central node is taken down by unfortunate events. 

Currently, I'm working on getting the encoder / decoder to work with raptorQ.

The current plan is:
1. Create raptorq encoder / decoder.
    - Eventually support handling bad peers. 
1. Have a centralized service manage CDN nodes.
1. Reduce reliability on the central service. 
    - Something like a DHT seems nice here. 

When the plan looks less hazy a spec will form. 