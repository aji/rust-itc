# `rust-itc`: Interval Tree Clocks

This is a Rust implementation of interval tree clocks, a causality tracking
mechanism for distributed systems where vector clocks are too limited, as is
the case of relatively frequent cluster updates in long-running systems.

[The paper describing ITC](http://gsd.di.uminho.pt/members/cbm/ps/itc2008.pdf)
explains the rationale and details of the mechanism.
