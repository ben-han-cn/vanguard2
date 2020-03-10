# Design decision
1. CName anwer should only trust/use the first level, we have no idea that resource   
   record from second level is authoried by current zone.

# Event statemachin
## States
- InitQuery 
- QueryTarget
- QueryResponse
- PrimeResponse
- TargetResponse
- Finished

## Query flow
1. InitQuery State
   * 1.1 if cache hit go to Finished(6)
   * 1.2 get DP(delegation point) from cache, if found go to QueryTarget(2)
   * 1.3 create prime root event, set DP from roothint go to QueryTarget(2)
2. QueryTarget State
   * 2.1 select host/server to query from DP, if not found go to 2.4
   * 2.2 send query to host, if get response go to QueryResponse(3)
   * 2.3 if the query is timeout return ServFail go to Finished(6)
   * 2.4 if there is missing nameserver which has no glue and isn't under the   
         zone of DP, create a new Query with server name as query name and A  
         as query type go to InitQuery(1) and set finish state to TargetResponse.  
         if not set response to ServFail, since there is not server to query.  
3. QueryResponse State
   * Classify response if get answer, cache it, then go to Finished(6)
   * if get referral, cache the response, then use it as new DP go to QueryTarget(2)
   * if get cname, cache the response, use the new name as query name go to InitQuery(1) 
   * otherwise, the server doesn't return right answer, go to QueryTarget(2) again
4. PrimeResponse State
   * Get the base event, if get response, use it as DP for base event, let base
     event go to QueryTarget(2)
   * If no response, set response to ServFail go to Finished(6)
5. TargetResponse State
   * if get answer from response, and the glue into DP of base event, resume 
     the base event, which will go to QueryTarget(2) state
   * set the queried server as probed, also let base event go to QueryTarget state
     which may probe another server.
6. Finished
   * generate the final response which involve merge the cname chain.

# HostSelector & Delegation Point
