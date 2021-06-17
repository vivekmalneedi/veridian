interface simple_bus (input logic clk); // Define the interface
    logic req, gnt;
    logic [7:0] addr, data;
    logic [1:0] mode;
    logic start, rdy;
    modport slave( input req, addr, mode, start, clk,
                   output gnt, rdy,
                   ref data,
                   export Read, Write);
    modport master(input gnt, rdy, clk,
                   output req, addr, mode, start,
                   ref data,
                   import task Read(input logic [7:0] raddr),
                   task Write(input logic [7:0] waddr));
endinterface: simple_bus

module test;
    logic clk;
    logic clk;
    simple_bus bus (.*);
endmodule
