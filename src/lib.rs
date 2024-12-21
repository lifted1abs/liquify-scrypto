mod liquify;
mod interface;

// Before release
// add in event stream stuff from oci/krullnull
// how can I plan ahead to include bot/autoatically creating new orders?
// plan ahead for upgradability - interface compponent?
// increase minimum liquidity to 10000
// test collecting of fills, what should max collect counter be set at?  Test both unstake nfts and lsu collects

// front end

// make chart display every .5% discount level until its too small then only every whole percentage
// direct x icon to link to https://x.com/liquifyxrd
// direct telegram icon to link to telegram group
// add button to "burn all closed orders" to the closed orders window if there are any orders visible
// add a button to "collect all fills" to the active orders window if there are any orders visible

// problems
// can iterate through 25 orders that auto unstake, 35 orders that do not auto unstake
// dont know where to cap collect fills
// dont know what transaction fees look like when book has large number of orders - thousands +

// testing considerations
// should more than .05% be available to select from?  infinite? 
// should the chart show
// add a burn all closed orders button to the closed order window
// when using min_order_size > 0, is it possible to skip over so many orders that the 25 cap is too big ie, failures because of hitting costing limit?  
// try calling this through an interface component and seeing what the iteration limit is