#include "BasicClient.h"
#include "slang_wrapper.h"
#include <iostream>
#include <slang/compilation/Compilation.h>
#include <slang/diagnostics/DiagnosticClient.h>
#include <slang/diagnostics/DiagnosticEngine.h>
#include <slang/diagnostics/Diagnostics.h>
#include <slang/parsing/Preprocessor.h>
#include <slang/types/TypePrinter.h>
#include <slang/syntax/SyntaxTree.h>
#include <slang/text/SourceManager.h>

using namespace slang;

char* compile_source(const char* name, const char* text) {
    Bag options;
    SourceManager sm;
    SourceBuffer buffer = sm.assignText(name, text);

    Compilation compilation(options);

    std::vector<SourceBuffer> buffers;
    buffers.push_back(buffer);
    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    std::string report = client->getString();
    char* report_c = strcpy(new char[report.length() + 1], report.c_str());
    return report_c;
}

char* compile_path(const char* path) {
    Bag options;
    SourceManager sm;
    SourceBuffer buffer = sm.readSource(path);

    Compilation compilation(options);

    std::vector<SourceBuffer> buffers;
    buffers.push_back(buffer);
    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    std::string report = client->getString();
    char* report_c = strcpy(new char[report.length() + 1], report.c_str());
    return report_c;
}

char* compile_sources(const char** names, const char** texts,
                      unsigned int num_files) {
    Bag options;
    SourceManager sm;
    Compilation compilation(options);

    std::vector<SourceBuffer> buffers;
    for (unsigned int i = 0; i < num_files; i++) {
        buffers.push_back(sm.assignText(names[i], texts[i]));
    }

    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    std::string report = client->getString();
    char* report_c = strcpy(new char[report.length() + 1], report.c_str());
    return report_c;
}

char* compile_paths(const char** paths, unsigned int num_paths) {
    Bag options;
    SourceManager sm;
    Compilation compilation(options);

    std::vector<SourceBuffer> buffers;
    for (unsigned int i = 0; i < num_paths; i++) {
        buffers.push_back(sm.readSource(paths[i]));
    }

    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    std::string report = client->getString();
    char* report_c = strcpy(new char[report.length() + 1], report.c_str());
    return report_c;
}

void delete_report(char* report) {
    delete report;
}
