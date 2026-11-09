#define _CRT_SECURE_NO_WARNINGS
#define SENGOO_STD_WRITER_TEST 1
#include "../runtime.c"
#ifdef _WIN32
#include <tlhelp32.h>
static long long buffer_create(size_t size) {
    long long token = sengoo_ffi_buffer_new((long long)size); assert(token > 0);
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(token); assert(buffer);
    for (size_t i = 0; i < size; i++) buffer->bytes[i] = (unsigned char)((i * 31 + i / 7) & 255);
    buffer->used_len = size; return token;
}
static void zero_resources(void) { for (int i = 0; i < 6; i++) assert(sengoo_io_writer_resource_count(i) == 0); }
static DWORD thread_count(void) {
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0); assert(snapshot != INVALID_HANDLE_VALUE);
    THREADENTRY32 entry = {0}; entry.dwSize = sizeof(entry); DWORD count = 0;
    if (Thread32First(snapshot, &entry)) do { if (entry.th32OwnerProcessID == GetCurrentProcessId()) count++; } while(Thread32Next(snapshot, &entry));
    CloseHandle(snapshot); return count;
}
static volatile LONG concurrent_owners;
static DWORD WINAPI registry_contender(void* argument) {
    (void)argument; int completed=0; ULONGLONG deadline=GetTickCount64()+15000;
    while(completed<40) {
        long long writer=sengoo_io_writer_new(1,1024);
        if(writer==-SENGOO_STATUS_ALREADY_EXISTS) { assert(GetTickCount64()<deadline); Sleep(0); continue; }
        assert(writer>0);
        assert(InterlockedIncrement(&concurrent_owners)==1);
        Sleep(0);
        assert(InterlockedDecrement(&concurrent_owners)==0);
        assert(sengoo_io_writer_close(writer)==0); completed++;
    }
    return 0;
}
static DWORD WINAPI registry_observer(void* argument) {
    HANDLE stopped=argument;
    while(WaitForSingleObject(stopped,0)==WAIT_TIMEOUT) {
        assert(sengoo_io_writer_resource_count(0)<=1);
        assert(sengoo_io_writer_resource_count(1)<=1);
        assert(sengoo_io_writer_resource_count(2)<=4);
        assert(sengoo_io_writer_resource_count(5)<=1024);
        Sleep(0);
    }
    return 0;
}
static void native_checks(void) {
    HANDLE original = GetStdHandle(STD_OUTPUT_HANDLE), read_pipe, write_pipe;
    assert(CreatePipe(&read_pipe, &write_pipe, NULL, 4096));
    assert(SetStdHandle(STD_OUTPUT_HANDLE, write_pipe));
    DWORD original_mode = 0; assert(GetNamedPipeHandleStateW(write_pipe,&original_mode,NULL,NULL,NULL,NULL,0));
    DWORD base_handles = 0; assert(GetProcessHandleCount(GetCurrentProcess(), &base_handles));
    DWORD base_threads = thread_count();
    FILE* legacy = tmpfile(); assert(legacy);
    unsigned char legacy_bytes[] = {0, 0xce, 0xbb}, legacy_read[3];
    assert(sengoo_io_write_stream(legacy,(long long)(intptr_t)legacy_bytes,3)==3);
    rewind(legacy); assert(fread(legacy_read,1,3,legacy)==3&&!memcmp(legacy_bytes,legacy_read,3)); fclose(legacy);
    long long buffer = buffer_create(65536);
    assert(sengoo_io_writer_new(1,1023)==-2 && sengoo_io_writer_new(1,1048577)==-2);
    for(int failure=1;failure<=3;failure++) {
        sengoo_std_writer_fail_stage=failure;
        assert(sengoo_io_writer_new(1,65536)<0); zero_resources();
    }
    sengoo_std_writer_fail_stage=0;
    long long stale=0;
    for(int round=0;round<40;round++) {
        long long writer=sengoo_io_writer_new(1,65536); assert(writer>0);
        assert(sengoo_io_writer_new(1,65536)==-6);
        if(stale) { assert(sengoo_io_writer_poll(stale,0)==-3); assert(sengoo_io_writer_close(stale)==-3); }
        assert(sengoo_io_writer_poll(writer+(1LL<<40),0)==-3);
        assert(sengoo_io_writer_try_write(writer,0,-1)==-2);
        assert(sengoo_io_writer_try_write(writer,0,65537)==-2);
        assert(sengoo_io_writer_try_write(writer,0,1)==-2);
        assert(sengoo_io_writer_try_write(writer,buffer,0)==0);
        if(round%4==0) {
            /* Cancel after a write has been selected but before native I/O is
             * issued: the first CancelSynchronousIo necessarily finds nothing. */
            sengoo_std_writer_before_write=CreateEventW(NULL,TRUE,FALSE,NULL);
            sengoo_std_writer_continue_write=CreateEventW(NULL,TRUE,FALSE,NULL);
        }
        if(round%4!=1) {
            assert(sengoo_io_writer_try_write(writer,buffer,65536)==65536);
            if(sengoo_std_writer_before_write) assert(WaitForSingleObject(sengoo_std_writer_before_write,2000)==WAIT_OBJECT_0);
            assert(sengoo_io_writer_try_write(writer,buffer,1)==0);
            assert(sengoo_io_writer_wait(writer,0)==0);
        }
        assert(sengoo_io_writer_stop(writer,1)==0);
        assert(sengoo_io_writer_try_write(writer,buffer,1)==-19);
        if(sengoo_std_writer_continue_write) {
            assert(sengoo_io_writer_wait(writer,0)==0);
            assert(sengoo_io_writer_resource_count(5)==65536);
            assert(sengoo_io_writer_resource_count(4)==65536);
        }
        if(sengoo_std_writer_continue_write) SetEvent(sengoo_std_writer_continue_write);
        assert(sengoo_io_writer_wait(writer,2000)==1);
        assert((sengoo_io_writer_poll(writer,0)&24)==24);
        assert(sengoo_io_writer_resource_count(0)==1);
        for(int i=1;i<6;i++) assert(sengoo_io_writer_resource_count(i)==0);
        if(sengoo_std_writer_before_write) {
            CloseHandle(sengoo_std_writer_before_write); CloseHandle(sengoo_std_writer_continue_write);
            sengoo_std_writer_before_write=sengoo_std_writer_continue_write=NULL;
        }
        assert(sengoo_io_writer_close(writer)==0); stale=writer; zero_resources();
        DWORD handles=0; assert(GetProcessHandleCount(GetCurrentProcess(),&handles)); assert(handles==base_handles);
        assert(thread_count()==base_threads);
        DWORD current_mode=0; assert(GetNamedPipeHandleStateW(write_pipe,&current_mode,NULL,NULL,NULL,NULL,0)); assert(current_mode==original_mode);
        /* A canceled write may already have transferred a prefix. Drain it so
         * the next round always starts with a known empty independent pipe. */
        DWORD available=0; assert(PeekNamedPipe(read_pipe,NULL,0,NULL,&available,NULL));
        while(available) { unsigned char bytes[4096]; DWORD got=0; assert(ReadFile(read_pipe,bytes,available>4096?4096:available,&got,NULL)); available-=got; }
    }
    sengoo_ffi_buffer_free(buffer);
    HANDLE stopped=CreateEventW(NULL,TRUE,FALSE,NULL), contenders[8]; assert(stopped);
    HANDLE observer=CreateThread(NULL,0,registry_observer,stopped,0,NULL); assert(observer);
    for(int i=0;i<8;i++) { contenders[i]=CreateThread(NULL,0,registry_contender,NULL,0,NULL); assert(contenders[i]); }
    assert(WaitForMultipleObjects(8,contenders,TRUE,20000)==WAIT_OBJECT_0);
    SetEvent(stopped); assert(WaitForSingleObject(observer,2000)==WAIT_OBJECT_0);
    for(int i=0;i<8;i++) CloseHandle(contenders[i]); CloseHandle(observer); CloseHandle(stopped);
    zero_resources(); DWORD handles=0; assert(GetProcessHandleCount(GetCurrentProcess(),&handles)&&handles==base_handles);
    assert(thread_count()==base_threads);
    assert(SetStdHandle(STD_OUTPUT_HANDLE,original)); CloseHandle(write_pipe); CloseHandle(read_pipe);
    /* Inherited regular files and message pipes cannot select this backend. */
    HANDLE file=CreateFileW(L"NUL",GENERIC_WRITE,FILE_SHARE_READ|FILE_SHARE_WRITE,NULL,OPEN_EXISTING,0,NULL);
    assert(file!=INVALID_HANDLE_VALUE); assert(SetStdHandle(STD_OUTPUT_HANDLE,file));
    assert(sengoo_io_writer_new(1,1024)==-8); assert(SetStdHandle(STD_OUTPUT_HANDLE,original)); CloseHandle(file);
    wchar_t name[128]; swprintf(name,128,L"\\\\.\\pipe\\sg-std-writer-%lu",GetCurrentProcessId());
    HANDLE message=CreateNamedPipeW(name,PIPE_ACCESS_OUTBOUND,PIPE_TYPE_MESSAGE|PIPE_READMODE_MESSAGE|PIPE_WAIT,1,4096,4096,0,NULL);
    assert(message!=INVALID_HANDLE_VALUE); assert(SetStdHandle(STD_OUTPUT_HANDLE,message));
    assert(sengoo_io_writer_new(1,1024)==-8); assert(SetStdHandle(STD_OUTPUT_HANDLE,original)); CloseHandle(message);
    zero_resources(); fprintf(stderr,"native checks passed: 40 exact resource rounds; 8 concurrent constructors x 40 with live snapshot observer\n");
}
static void blocked_child(void) {
    long long buffer=buffer_create(65536), writer=sengoo_io_writer_new(1,65536); assert(writer>0);
    assert(sengoo_io_writer_try_write(writer,buffer,65536)==65536);
    assert(sengoo_io_writer_try_write(writer,buffer,1)==0);
    ULONGLONG issued_deadline=GetTickCount64()+2000;
    for(;;) {
        BOOL pending=FALSE;
        assert(GetThreadIOPendingFlag(sengoo_std_writer_resolve(writer)->thread,&pending));
        if(pending) break;
        assert(GetTickCount64()<issued_deadline); Sleep(1);
    }
    fprintf(stderr,"READY\n"); fflush(stderr);
    char command=0; DWORD got=0;
    assert(ReadFile(GetStdHandle(STD_INPUT_HANDLE),&command,1,&got,NULL)&&got==1&&command=='c');
    ULONGLONG start=GetTickCount64(); assert(sengoo_io_writer_stop(writer,1)==0);
    assert(sengoo_io_writer_wait(writer,2000)==1);
    long long snapshot=buffer_create(64); assert(sengoo_io_writer_snapshot(writer,snapshot)==0);
    assert(sengoo_io_writer_snapshot_get(snapshot,0)==5);
    assert(sengoo_io_writer_snapshot_get(snapshot,3)==0&&sengoo_io_writer_snapshot_get(snapshot,4)==0);
    assert(sengoo_io_writer_close(writer)==0); zero_resources();
    fprintf(stderr,"CANCELED %llu\n",GetTickCount64()-start); sengoo_ffi_buffer_free(snapshot); sengoo_ffi_buffer_free(buffer);
}
static void binary_child(int endpoint) {
    long long buffer=buffer_create(8192), writer=sengoo_io_writer_new(endpoint,1024); assert(writer>0);
    SengooFfiBuffer* source=sengoo_ffi_buffer_from_handle(buffer); size_t offset=0; ULONGLONG deadline=GetTickCount64()+10000;
    while(offset<8192) {
        long long accepted=sengoo_io_writer_try_write(writer,buffer,8192-(long long)offset); assert(accepted>=0);
        if(accepted) { offset+=(size_t)accepted; memmove(source->bytes,source->bytes+accepted,source->used_len-(size_t)accepted); source->used_len-=(size_t)accepted; }
        else { assert(GetTickCount64()<deadline); Sleep(1); }
    }
    assert(sengoo_io_writer_stop(writer,0)==0); assert(sengoo_io_writer_wait(writer,5000)==1);
    long long snapshot=buffer_create(64); assert(sengoo_io_writer_snapshot(writer,snapshot)==0);
    assert(sengoo_io_writer_snapshot_get(snapshot,1)==8192&&sengoo_io_writer_snapshot_get(snapshot,2)==8192);
    assert(sengoo_io_writer_snapshot_get(snapshot,6)==0); assert(sengoo_io_writer_close(writer)==0);
    sengoo_ffi_buffer_free(snapshot); sengoo_ffi_buffer_free(buffer); zero_resources(); fprintf(endpoint==1?stderr:stdout,"BINARY OK\n");
}
static void broken_child(void) {
    long long buffer=buffer_create(65536),writer=sengoo_io_writer_new(1,65536); assert(writer>0);
    fprintf(stderr,"READY\n"); fflush(stderr); char c; DWORD n; assert(ReadFile(GetStdHandle(STD_INPUT_HANDLE),&c,1,&n,NULL));
    assert(sengoo_io_writer_try_write(writer,buffer,65536)==65536);
    assert(sengoo_io_writer_wait(writer,2000)==1); assert((sengoo_io_writer_poll(writer,0)&4)==4);
    assert(sengoo_io_writer_try_write(writer,buffer,1)<0);
    assert(sengoo_io_writer_close(writer)==0); sengoo_ffi_buffer_free(buffer); zero_resources(); fprintf(stderr,"BROKEN OK\n");
}
#endif
int main(int argc,char** argv) {
#ifdef _WIN32
    assert(argc==2);
    if(!strcmp(argv[1],"native")) native_checks();
    else if(!strcmp(argv[1],"blocked")) blocked_child();
    else if(!strcmp(argv[1],"binary")) binary_child(1);
    else if(!strcmp(argv[1],"binary-stderr")) binary_child(2);
    else if(!strcmp(argv[1],"broken")) broken_child();
    else assert(0);
#else
    (void)argc; (void)argv; assert(sengoo_io_writer_new(1,1024)==-SENGOO_STATUS_UNSUPPORTED);
#endif
    return 0;
}
