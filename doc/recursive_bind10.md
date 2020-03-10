# Recursive query
1. lookup message cahce, if answer section isn't empty return the message
2. lookup rrset cache, if has rrset, build the response and return
3. lookup message cache again, 
   1. if found use it as a intermidiate response goto step 4
   1. else use cache to find the possible closest enclosure zone as `current_zone` and go to step 5
4. classify the response and handle it
   1. Answer, AnswerCName  
      1. update cache
      1. use it as response and return
   1. CName  
      1. if cname is too deep return server failed
      1. merge the answer
      1. use the new name as new target and go to step 3
   1. NXDomain & NXRRset   
      1. use it as response
   1. Referal  
      1. update the cache 
      1. if return ns record is current_zone's subdomain, use it as new `current_zone`
         - goto step 5
      1. else return the response
   1. truncate answer  
      1. if current protocol is udp, use tcp to send
      1. else return server failed
   1. Format error  
      1. if current protocol is udp and edns used, retry with edns disable
      1. else return server failed
5. use NSAS to find the address of nameserver of `current_zone`, if not found, return server failed
, else send the query to the nameserver
   1. if get response, goto step 4
   1. else return server failed

# Cache
## RRset Cache
1. A LRU of rrset entry
1. RRset entry consist of the rrset and trust level  

## Message Cache
1. A LRU of message entry
1. A message entry consist of a query(name + qutyp) and point to rrset in rrset cache
1. There are two rrset cache, one for positive answer, the other stores SOA record in
   the negative answer

# NSAS
1.  AddressEntry
  - ip address(v4 + v6) 
  - rtt of the address
1.  NameserverEntry: 
  - name of the name server which is the rdata part of the zone NS record
  - addresses of the name server which is built from A/AAAA record
1.  ZoneEntry
  - zone name
  - several name servers

```
                                              +---------+
                                       +------>address  |
                                       |      |---------|
                                       |      |ipaddress|
                      +----------+     |      |rtt      |
                  +--->nameserver|     |      +---------+
                  |   |----------|     |
                  |   |name      |     |      +---------+
                  |   |expiration|     +------>address  |
                  |   |addresses +-----+      |---------|
                  |   +----------+     |      |ipaddress|
                  |                    |      |rtt      |
   +-----------+  |   +----------+     |      +---------+
   |zone       |  +--->nameserver|     |
   |-----------|  |   |----------|     |      +---------+
   |name       |  |   |name      |     +------>address  |
   |nameservers+--+   |expiration|            |---------|
   +-----------+  |   |addresses |            |ipaddress|
                  |   +----------+            |rtt      |
                  |                           +---------+
                  |   +----------+
                  +--->nameserver|
                      |----------|
                      |name      |
                      |expiration|
                      |addresses |
                      +----------+
```
