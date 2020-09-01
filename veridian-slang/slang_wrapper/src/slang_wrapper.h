#pragma once

extern "C" {
void delete_report(char* report);
char* compile_source(const char* name, const char* text);
char* compile_path(const char* path);
char* compile_sources(const char** names, const char** texts,
                      unsigned int num_files);
char* compile_paths(const char** paths, unsigned int num_paths);
}
