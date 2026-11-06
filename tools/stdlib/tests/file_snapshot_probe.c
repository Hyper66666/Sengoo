#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#include <winioctl.h>
static int read_mode, read_calls, fragment_reads = 1;
static wchar_t guarded_file[32768], guarded_directory[32768], moved_directory[32768];
static BOOL WINAPI probe_read(HANDLE file, LPVOID bytes, DWORD count, LPDWORD got, LPOVERLAPPED op) {
    read_calls++;
    if (read_mode == 1 && read_calls > 1) { SetLastError(ERROR_READ_FAULT); return FALSE; }
    if (read_mode == 2 && read_calls == 1) {
        HANDLE writer = CreateFileW(guarded_file, GENERIC_WRITE, FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
        assert(writer == INVALID_HANDLE_VALUE && GetLastError() == ERROR_SHARING_VIOLATION);
        assert(!DeleteFileW(guarded_file) && GetLastError() == ERROR_SHARING_VIOLATION);
        assert(!MoveFileW(guarded_directory, moved_directory));
        assert(GetLastError() == ERROR_SHARING_VIOLATION || GetLastError() == ERROR_ACCESS_DENIED);
    }
    if (read_mode == 3 && read_calls > 1) { *got = 0; return TRUE; }
    if (read_mode == 4 && count == 1 && read_calls > 1) { *(unsigned char*)bytes = 1; *got = 1; return TRUE; }
    if (fragment_reads && count > 1) count = 1; /* Exercise fragmented complete reads. */
    return ReadFile(file, bytes, count, got, op);
}
static BOOL WINAPI probe_information(HANDLE file, LPBY_HANDLE_FILE_INFORMATION info) {
    BOOL result = GetFileInformationByHandle(file,info);
    if(result && read_mode == 5 && read_calls > 0 && !(info->dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)) info->ftLastWriteTime.dwLowDateTime++;
    return result;
}
#define ReadFile probe_read
#define GetFileInformationByHandle probe_information
#endif
#include "../runtime_file_snapshot.c"
#ifdef _WIN32
#undef ReadFile
#undef GetFileInformationByHandle
#endif
long long sengoo_opaque_handle_new(void* value) { return (long long)(intptr_t)value; }
void* sengoo_opaque_handle_take(long long handle) { return (void*)(intptr_t)handle; }
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long handle) { return (SengooFfiBuffer*)(intptr_t)handle; }
long long sengoo_string_from_bytes_copy(long long bytes, long long length) {
    char* text = malloc((size_t)length + 1); assert(text);
    memcpy(text, (void*)(intptr_t)bytes, (size_t)length); text[length] = 0; return (long long)(intptr_t)text;
}
static long long raw(const void* pointer) { return (long long)(intptr_t)pointer; }
#ifdef _WIN32
static void write_file(const wchar_t* name, const void* bytes, DWORD length) {
    HANDLE file = CreateFileW(name, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL); assert(file != INVALID_HANDLE_VALUE);
    DWORD wrote = 0; assert(WriteFile(file, bytes, length, &wrote, NULL) && wrote == length); assert(CloseHandle(file));
}
static void convert(const wchar_t* input, char* output) { assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, input, -1, output, 65536, NULL, NULL)); }
static long long open_snapshot(const char* path, long long limit) { read_calls = 0; return sengoo_file_snapshot_open(raw(path), (long long)strlen(path), limit); }
static void text_equal(long long owner, long long field, const char* expected) {
    long long result = sengoo_file_snapshot_text(owner, field); assert(result > 0); assert(!strcmp((char*)(intptr_t)result, expected)); free((void*)(intptr_t)result);
}
static void run_child(const wchar_t* executable) {
    static wchar_t command[32768]; swprintf(command,32768,L"\"%ls\" --child",executable);
    STARTUPINFOW startup={0}; startup.cb=sizeof(startup); PROCESS_INFORMATION process={0};
    assert(CreateProcessW(executable,command,NULL,NULL,FALSE,CREATE_NO_WINDOW,NULL,NULL,&startup,&process)); assert(CloseHandle(process.hThread));
    assert(WaitForSingleObject(process.hProcess,5000)==WAIT_OBJECT_0); DWORD code=1; assert(GetExitCodeProcess(process.hProcess,&code)&&code==0); assert(CloseHandle(process.hProcess));
}
static void junction(const wchar_t* link, const wchar_t* target) {
    assert(CreateDirectoryW(link, NULL));
    HANDLE directory = CreateFileW(link, GENERIC_WRITE, 0, NULL, OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT|FILE_FLAG_BACKUP_SEMANTICS, NULL); assert(directory != INVALID_HANDLE_VALUE);
    struct { DWORD tag; WORD length, reserved, substitute_offset, substitute_length, print_offset, print_length; wchar_t text[32768]; } buffer = {0};
    buffer.tag = IO_REPARSE_TAG_MOUNT_POINT;
    int count = swprintf(buffer.text, 32768, L"\\??\\%ls", target); assert(count > 0);
    buffer.substitute_length = (WORD)(count * sizeof(wchar_t));
    buffer.print_offset = (WORD)((count + 1) * sizeof(wchar_t));
    int shown = swprintf(buffer.text + count + 1, 32768 - count - 1, L"%ls", target); assert(shown > 0);
    buffer.print_length = (WORD)(shown * sizeof(wchar_t));
    buffer.length = 8 + buffer.substitute_length + buffer.print_length + 4;
    DWORD returned = 0; assert(DeviceIoControl(directory, FSCTL_SET_REPARSE_POINT, &buffer, buffer.length + 8, NULL, 0, &returned, NULL)); assert(CloseHandle(directory));
}
#endif
int main(int argc, char** argv) {
    if (argc == 2 && !strcmp(argv[1],"--child")) return 0;
#ifdef _WIN32
    static wchar_t temporary[32768], file[32768], empty[32768], nested[32768], link[32768], through[32768];
    assert(GetTempPathW(32700, temporary));
    swprintf(guarded_directory, 32768, L"%lsSengoo-snapshot-\x6587 \xd83d\xde00-%lu", temporary, GetCurrentProcessId());
    swprintf(moved_directory, 32768, L"%ls-moved", guarded_directory); assert(CreateDirectoryW(guarded_directory, NULL));
    swprintf(file, 32768, L"%ls\\binary.bin", guarded_directory); wcscpy(guarded_file, file);
    swprintf(empty, 32768, L"%ls\\empty.bin", guarded_directory); write_file(file, "a\0b", 3); write_file(empty, "", 0);
    static char path[65536], empty_path[65536], directory_path[65536]; convert(file, path); convert(empty, empty_path); convert(guarded_directory, directory_path);
    static wchar_t self[32768], copied_exe[32768]; static char exe_path[65536];
    assert(GetModuleFileNameW(NULL,self,32768)); swprintf(copied_exe,32768,L"%ls\\worker.exe",guarded_directory); assert(CopyFileW(self,copied_exe,TRUE)); convert(copied_exe,exe_path);
    /* CreateProcess of a Volume-GUID path initializes persistent OS bookkeeping
     * on this host. Warm the same OS path mode before checking owner stability. */
    HANDLE warm = CreateFileW(copied_exe,FILE_READ_ATTRIBUTES,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,0,NULL); assert(warm!=INVALID_HANDLE_VALUE);
    static wchar_t warm_path[32768]; assert(GetFinalPathNameByHandleW(warm,warm_path,32768,VOLUME_NAME_GUID)); assert(CloseHandle(warm)); run_child(warm_path);
    DWORD baseline = 0; assert(GetProcessHandleCount(GetCurrentProcess(), &baseline));
    assert(sengoo_file_snapshot_available() == 1);
    read_mode = 2; long long owner = open_snapshot(path, 3); assert(owner > 0 && sengoo_file_snapshot_length(owner) == 3);
    assert(sengoo_file_snapshot_live_handle_count() == 1 && sengoo_file_snapshot_live_bytes() == 3);
    assert(sengoo_file_snapshot_still_matches(owner) == 1);
    text_equal(owner, 0, "59b271ae1bbcb1d31d41929817f4b16fb439eb4f31520b5ad1d5ce98920a7138");
    long long pinned = sengoo_file_snapshot_text(owner, 3); assert(pinned > 0 && !strncmp((char*)(intptr_t)pinned,"\\\\?\\Volume{",11)); free((void*)(intptr_t)pinned);
    unsigned char output[4] = {0}; SengooFfiBuffer buffer = {0}; buffer.bytes = output; buffer.capacity = 2;
    assert(sengoo_file_snapshot_copy_bytes(owner, raw(&buffer)) == -SENGOO_STATUS_BUFFER_TOO_SMALL && buffer.used_len == 0);
    buffer.capacity = sizeof(output); assert(sengoo_file_snapshot_copy_bytes(owner, raw(&buffer)) == 3 && buffer.used_len == 3 && !memcmp(output,"a\0b",3));
    assert(sengoo_file_snapshot_close(owner) == 0 && sengoo_file_snapshot_length(owner) == -SENGOO_STATUS_INVALID_HANDLE);
    read_mode = 0; assert(open_snapshot(path, 2) == -SENGOO_STATUS_OVERFLOW);
    assert(open_snapshot(path, -1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(open_snapshot(path, 64LL*1024*1024+1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    owner = open_snapshot(empty_path, 0); assert(owner > 0 && sengoo_file_snapshot_length(owner) == 0);
    text_equal(owner,0,"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"); assert(sengoo_file_snapshot_close(owner) == 0);
    assert(open_snapshot(directory_path, 1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(open_snapshot("NUL", 1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(open_snapshot("C:relative.bin",1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(open_snapshot("\\\\.\\NUL",1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    static char alternate[65536]; snprintf(alternate,sizeof(alternate),"%s:stream",path); assert(open_snapshot(alternate,1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    const unsigned char invalid[] = {0xc0,0x80}; assert(sengoo_file_snapshot_open(raw(invalid),2,10) == -SENGOO_STATUS_INVALID_UTF8);
    assert(sengoo_file_snapshot_open(raw("x\0y"),3,10) == -SENGOO_STATUS_INVALID_ARGUMENT);
    HANDLE writer = CreateFileW(file, GENERIC_WRITE, FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,0,NULL); assert(writer != INVALID_HANDLE_VALUE);
    assert(open_snapshot(path,3) == -SENGOO_STATUS_PERMISSION_DENIED); assert(CloseHandle(writer));
    for (int mode = 1; mode <= 5; mode++) if (mode != 2) {
        read_mode = mode; long long expected = mode == 4 ? -SENGOO_STATUS_OVERFLOW : -SENGOO_STATUS_IO;
        for(int i=0;i<32;i++) assert(open_snapshot(path,3) == expected);
        assert(sengoo_file_snapshot_live_handle_count() == 0 && sengoo_file_snapshot_live_bytes() == 0);
    }
    read_mode = 0;
    long long owners[32];
    for(int i=0;i<32;i++){ owners[i]=open_snapshot(empty_path,0); assert(owners[i]>0); }
    assert(open_snapshot(empty_path,0)==-SENGOO_STATUS_OVERFLOW);
    for(int i=0;i<32;i++) assert(sengoo_file_snapshot_close(owners[i])==0);
    fragment_reads=0; owner=open_snapshot(exe_path,64LL*1024*1024); assert(owner>0);
    pinned=sengoo_file_snapshot_text(owner,3); assert(pinned>0);
    static wchar_t launch[32768]; assert(MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,(const char*)(intptr_t)pinned,-1,launch,32768));
    free((void*)(intptr_t)pinned); run_child(launch); run_child(launch); run_child(launch);
    assert(sengoo_file_snapshot_close(owner)==0 && DeleteFileW(copied_exe)); fragment_reads=1;
    swprintf(nested,32768,L"%ls\\child",guarded_directory); assert(CreateDirectoryW(nested,NULL));
    swprintf(link,32768,L"%ls\\junction",guarded_directory); junction(link,nested);
    swprintf(through,32768,L"%ls\\other.bin",link);
    static wchar_t real_child[32768]; swprintf(real_child,32768,L"%ls\\other.bin",nested); write_file(real_child,"x",1);
    static char junction_path[65536], through_path[65536]; convert(link,junction_path); convert(through,through_path);
    assert(open_snapshot(junction_path,10) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(open_snapshot(through_path,10) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(RemoveDirectoryW(link) && DeleteFileW(real_child) && RemoveDirectoryW(nested));
    owner = open_snapshot(path,3); assert(owner > 0); assert(sengoo_file_snapshot_close(owner) == 0);
    assert(MoveFileW(guarded_directory,moved_directory) && MoveFileW(moved_directory,guarded_directory));
    DWORD after; assert(GetProcessHandleCount(GetCurrentProcess(),&after) && after <= baseline + 2);
    assert(DeleteFileW(file) && DeleteFileW(empty) && RemoveDirectoryW(guarded_directory));
    puts("snapshot native: exact bytes/hash, Unicode, pinned executable launch/ancestor exclusion, caps, failures, reparse and handle cleanup passed");
#else
    assert(sengoo_file_snapshot_available() == 0);
    assert(sengoo_file_snapshot_open(0,0,0) == -SENGOO_STATUS_UNSUPPORTED);
#endif
    return 0;
}
