# raptorCDN

The overall goal is to provide a somewhat-decentralized P2P CDN based on raptor codes. At the minimum, be able to recover quickly from the network if the central node is taken down by unfortunate events. 

Currently, this is stalled since I'm noticing relatively poor performance, requiring chunking files into blocks. At that point, it almost seems preferable to just use the BitTorrent protocol. 

The current plan is:
1. Create raptorq encoder / decoder.
    - Eventually support handling bad peers. 
1. Have a centralized service manage CDN nodes.
1. Reduce reliability on the central service. 
    - Something like a DHT seems nice here. 

When the plan looks less hazy a spec will form. 