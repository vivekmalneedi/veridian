#include "BasicClient.h"
#include "slang_wrapper.h"
#include <array>
#include <iostream>
#include <filesystem>
#include <fmt/format.h>
#include <slang/ast/Compilation.h>
#include <slang/ast/types/TypePrinter.h>
#include <slang/diagnostics/DiagnosticClient.h>
#include <slang/diagnostics/DiagnosticEngine.h>
#include <slang/diagnostics/Diagnostics.h>
#include <slang/parsing/Preprocessor.h>
#include <slang/syntax/SyntaxTree.h>
#include <slang/text/SourceManager.h>

namespace fs = std::filesystem;

using namespace slang;
using slang::syntax::SyntaxTree;

// Private function
static char* report(const std::string &s) {
    return strcpy(new char[s.length() + 1], s.c_str());
}

char* compile_source(const char* name, const char* text) {
    Bag options;
    SourceManager sm;
    SourceBuffer buffer = sm.assignText(name, text);

    ast::Compilation compilation(options);

    std::array<SourceBuffer, 1> buffers{std::move(buffer)};
    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    return report(client->getString());
}

char* compile_path(const char* path) {
    Bag options;
    SourceManager sm;
    auto buffer = sm.readSource(fs::path{path}, /* library */ nullptr);
    if (!buffer) {
        return report(fmt::format("'{}': {}", path, buffer.error().message()));
    }

    ast::Compilation compilation(options);

    std::array<SourceBuffer, 1> buffers{std::move(*buffer)};
    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    return report(client->getString());
}

char* compile_sources(const char** names, const char** texts,
                      unsigned int num_files) {
    Bag options;
    SourceManager sm;
    ast::Compilation compilation(options);

    std::vector<SourceBuffer> buffers;
    buffers.reserve(num_files);
    for (unsigned int i = 0; i < num_files; i++) {
        buffers.emplace_back(sm.assignText(names[i], texts[i]));
    }

    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    return report(client->getString());
}

char* compile_paths(const char** paths, unsigned int num_paths) {
    Bag options;
    SourceManager sm;
    ast::Compilation compilation(options);

    std::vector<SourceBuffer> buffers;
    buffers.reserve(num_paths);
    for (unsigned int i = 0; i < num_paths; i++) {
        auto buffer = sm.readSource(fs::path{paths[i]}, /* library */ nullptr);
        if (!buffer) {
            return report(fmt::format("'{}': {}", paths[i], buffer.error().message()));
        }
        buffers.emplace_back(std::move(*buffer));
    }

    compilation.addSyntaxTree(SyntaxTree::fromBuffers(buffers, sm, options));

    DiagnosticEngine diagEngine(sm);
    Diagnostics pragmaDiags = diagEngine.setMappingsFromPragmas();

    auto client = std::make_shared<BasicClient>();
    client->setColorsEnabled(false);
    diagEngine.addClient(client);

    for (auto& diag : compilation.getAllDiagnostics()) diagEngine.issue(diag);

    return report(client->getString());
}

void delete_report(char* report) {
    delete report;
}
