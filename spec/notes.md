
RaptorQEndoer should take in a [task / work queue thingy] and some data to encode, where it will create encoded data of size equal to the input. These blocks will be shipped async to some client (pref web client). Note that the block encoding can take some time, and so multiprocessing with rayon is preferred to encode large datasets. 

RaptorQDecoder should be made so that it can be incorporated with web-code via webasm. Or something. I don't know how web stuff works. 