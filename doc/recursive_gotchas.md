1. recusive answer cann't be trust
- out of zone rrset
- nxdomaon or serverfail rcode but has answer
- bind only trust the first level cname, but vanguard2 trust all the 
  cname chain under current zone.
1. glue record has cname
1. ns isn't expired in cache, but all glue which under zone is expired
1. nameserver doesn't support edns
