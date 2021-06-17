module test;
endmodule

program test;
endprogram

interface test;
endinterface

interface test1(clk);
endinterface

checker test;
    covergroup group;
    endgroup
endchecker

task test;
endtask

function test;
endfunction

package test;
endpackage

class test;
endclass

enum {red, yellow, green} light1, light2;

struct { bit [7:0] opcode; bit [23:0] addr; } IR1, IR2;

typedef union { int i; shortreal f; } num;

logic a,b;

module test(
    input logic [1:0] clk [2]
);
endmodule

module test(clk);
    input logic clk2;
    inter.a clk3;
endmodule
