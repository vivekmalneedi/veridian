[
(module_declaration
    (_
        (module_keyword) @keyword
        (simple_identifier) @ident))
(program_declaration
    (_
        "program" @keyword
        (program_identifier) @ident))
(interface_declaration
    (_
        "interface" @keyword
        (interface_identifier) @ident))
(checker_declaration
    "checker" @keyword
    (checker_identifier) @ident)
(covergroup_declaration
    "covergroup" @keyword
    (covergroup_identifier) @ident)
(task_declaration
    "task" @keyword
    (_
        (task_identifier) @ident))
(function_declaration
    "function" @keyword
    (_
        (function_identifier) @ident))
(package_declaration
    "package" @keyword
    (package_identifier) @ident)
(class_declaration
    "class" @keyword
    (class_identifier) @ident)
(data_declaration
    (data_type_or_implicit1
        (data_type
            "enum" @keyword))
    (list_of_variable_decl_assignments
        (variable_decl_assignment
            (simple_identifier) @ident)))
(data_declaration
    [
    (data_type_or_implicit1
        (data_type
            (struct_union) @keyword))
    (type_declaration
        (data_type
            (struct_union) @keyword)
        (simple_identifier) @ident)
    ]
    (list_of_variable_decl_assignments
        (variable_decl_assignment
            (simple_identifier) @ident))?)
(udp_declaration
    [
    (udp_nonansi_declaration
        "primitive" @keyword
        (simple_identifier) @ident)
    (udp_ansi_declaration
        "primitive" @keyword
        (simple_identifier) @ident)
    ])
] @scope

[
(ansi_port_declaration
    [
    (variable_port_header
        (port_direction)? @direction
        [
        (data_type)? @type
        ("var"? @type (data_type_or_implicit1))
        ]?)
    (net_port_header1
        (port_direction)? @direction
        (net_port_type1
            [
            (data_type_or_implicit1
                [
                (data_type) @type
                (implicit_data_type1
                    "signed" @signed)
                ])
            (net_type) @type
            (simple_identifier) @type
            ])?)
    (interface_port_header
        (interface_identifier) @interface
        (modport_identifier)? @modport) @type
    (port_direction) @direction
    ]?
    (port_identifier) @ident)
(port_declaration
    (_
        ["input" "output" "inout" "ref"] @direction
        [
        ("var"? @type (data_type_or_implicit1))
        (net_port_type1
            [
            (data_type_or_implicit1
                [
                (data_type) @type
                (implicit_data_type1
                    "signed" @signed)
                ])
            (net_type) @type
            (simple_identifier) @type
            ])
            (data_type_or_implicit1
                [
                (data_type) @type
                (implicit_data_type1
                    "signed" @signed)
                ])
        (data_type) @type
        ]?
        [
        (list_of_port_identifiers
            (port_identifier) @ident)
        (list_of_variable_identifiers
            (simple_identifier) @ident)
        ]))
(port_declaration
    (interface_port_declaration
        (interface_identifier) @interface
        (modport_identifier)? @modport
        (list_of_interface_identifiers
            (interface_identifier) @ident)))
(udp_output_declaration
    "output" @direction
    (port_identifier) @ident)
(udp_reg_declaration
    "reg" @type
    (simple_identifier) @ident)
(udp_input_declaration
    "input" @direction
    (list_of_udp_port_identifiers
        (port_identifier) @ident))
(tf_port_item1
    (tf_port_direction)? @direction
    (data_type_or_implicit1
        [
        (data_type
            (simple_identifier)? @ident) @type
        (implicit_data_type1
            "signed" @signed)
        ])?
    (port_identifier)? @ident)
(tf_port_declaration
    (tf_port_direction) @direction
    (list_of_tf_variable_identifiers
        (port_identifier) @ident))
] @port

[
(parameter_declaration
    "parameter" @type
    [
    (list_of_param_assignments
        (param_assignment
            (parameter_identifier) @ident))
    (list_of_type_assignments
        (type_assignment
            (simple_identifier) @ident))
    ])
(local_parameter_declaration
    "localparam" @type
    [
    (list_of_param_assignments
        (param_assignment
            (parameter_identifier) @ident))
    (list_of_type_assignments
        (type_assignment
            (simple_identifier) @ident))
    ])
(parameter_port_declaration
    [
    (data_type
        (simple_identifier) @ident)
    (list_of_param_assignments
        (param_assignment
            (parameter_identifier) @ident))
    ])
(parameter_port_list
    (list_of_param_assignments
        (param_assignment
            (parameter_identifier) @ident)))
] @param

(package_import_declaration
    (package_import_item
        (package_identifier) @package
        [
        (simple_identifier)
        "*"
        ] @ident)) @import

(struct_union_member
    (data_type_or_void) @type
    (list_of_variable_decl_assignments
        (variable_decl_assignment
            (simple_identifier) @ident)))

[
(udp_instantiation
    (simple_identifier) @type
    (udp_instance
        (name_of_instance) @ident))
(checker_instantiation
    (checker_identifier) @type
    (name_of_instance) @ident)
(module_instantiation
    (simple_identifier) @type
    (hierarchical_instance
        (name_of_instance) @ident))
] @instantiation

[
(data_declaration
    [
    (data_type_or_implicit1
        (data_type
            [
            (simple_identifier)
            (integer_atom_type)
            (integer_vector_type)
            (interface_identifier)
            (modport_identifier)
            (non_integer_type)
            "chandle"
            "string"
            ] @type))
    "var" @type
    ]
    (list_of_variable_decl_assignments
        (variable_decl_assignment
            (simple_identifier) @ident)))
(data_declaration
    (type_declaration
        (data_type
            [
            (simple_identifier)
            (integer_atom_type)
            (integer_vector_type)
            (interface_identifier)
            (modport_identifier)
            (non_integer_type)
            "chandle"
            "string"
            ] @type)
        (simple_identifier) @ident))
(data_declaration
    (net_type_declaration
        (data_type) @type
        (simple_identifier) @ident))
(net_declaration
    (net_type) @type
    (list_of_net_decl_assignments
        (net_decl_assignment
            (simple_identifier) @ident)))
] @variable
