interface test(logic clk);
    logic a;
    logic b;
    modport in (input clk, a , b);
    modport out (output clk, a, b);
endinterface

module test1(
    test.in tinter,
    test interb
);
endmodule
