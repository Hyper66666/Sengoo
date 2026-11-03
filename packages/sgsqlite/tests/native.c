#include "native_runtime.h"
#include "../native/sgsqlite.c"
/* The amalgamation defines NDEBUG for its own internal assertions. */
#undef NDEBUG
#include <assert.h>
static long long count(long long db) {
    long long s=sgsqlite_prepare(db,test_pointer("SELECT count(*) FROM items"));assert(s>0);
    assert(sgsqlite_step(s)==1);long long n=sgsqlite_column_i64(s,0);assert(sgsqlite_error_status()==0);sgsqlite_stmt_close(s);return n;
}
int main(void) {
    test_string(sgsqlite_version(),"3.54.1");
    const char *path="sgsqlite-native.db";remove(path);
    long long db=sgsqlite_open(test_pointer(path));assert(db>0);
    assert(sgsqlite_busy_timeout(db,25)==0);
    assert(sgsqlite_exec(db,test_pointer("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT, bytes BLOB)"))==0);
    long long s=sgsqlite_prepare(db,test_pointer("INSERT INTO items VALUES(?,?,?)"));assert(s>0);
    assert(sgsqlite_bind_i64(s,1,LLONG_MAX)==0);
    const char *name="\xe4\xb8\x96\xe7\x95\x8c'; DROP TABLE items; --";
    assert(sgsqlite_bind_bytes(s,2,test_pointer(name),(long long)strlen(name),1)==0);
    const char bytes[]={0,1,(char)255};assert(sgsqlite_bind_bytes(s,3,test_pointer(bytes),3,0)==0);
    assert(sgsqlite_step(s)==0);assert(sgsqlite_stmt_close(s)==0);assert(sgsqlite_stmt_close(s)==-3);
    assert(count(db)==1);
    s=sgsqlite_prepare(db,test_pointer("SELECT id,name,bytes,NULL FROM items"));assert(s>0);assert(sgsqlite_step(s)==1);
    assert(sgsqlite_column_i64(s,0)==LLONG_MAX);
    test_string(sgsqlite_column_text(s,1),name);
    long long blob=sgsqlite_column_blob(s,2);TestBytes *b=(TestBytes*)(intptr_t)blob;assert(b->size==3&&!memcmp(b->data,bytes,3));
    assert(sgsqlite_column_type(s,3)==SQLITE_NULL);
    long long copied=sgsqlite_column_text(s,1);assert(sgsqlite_step(s)==0);test_string(copied,name);test_buffer_free(blob);sgsqlite_stmt_close(s);
    assert(sgsqlite_exec(db,test_pointer("BEGIN"))==-7);
    long long tx=sgsqlite_begin(db);assert(tx>0);assert(sgsqlite_begin(db)==-7);
    assert(sgsqlite_exec(db,test_pointer("DELETE FROM items"))==-7);
    assert(sgsqlite_tx_exec(tx,test_pointer("INSERT INTO items VALUES(1,'rollback',NULL)"))==0);
    assert(sgsqlite_tx_close(tx)==0);assert(count(db)==1);
    tx=sgsqlite_begin(db);assert(tx>0);assert(sgsqlite_tx_exec(tx,test_pointer("INSERT INTO items VALUES(1,'commit',NULL)"))==0);
    assert(sgsqlite_commit(tx)==0);assert(sgsqlite_commit(tx)==-7);sgsqlite_tx_close(tx);assert(count(db)==2);
    tx=sgsqlite_begin(db);assert(tx>0);assert(sgsqlite_tx_exec(tx,test_pointer("INSERT INTO items VALUES(2,'before error',NULL)"))==0);
    assert(sgsqlite_tx_exec(tx,test_pointer("INSERT INTO items VALUES(1,'duplicate',NULL)"))==-2);
    assert(sgsqlite_error_code()==SQLITE_CONSTRAINT_PRIMARYKEY);assert(sgsqlite_commit(tx)==-7);sgsqlite_tx_close(tx);assert(count(db)==2);
    long long second=sgsqlite_open(test_pointer(path));assert(second>0);assert(sgsqlite_busy_timeout(second,5)==0);
    tx=sgsqlite_begin(db);assert(tx>0);assert(sgsqlite_tx_exec(tx,test_pointer("INSERT INTO items VALUES(3,'locked',NULL)"))==0);
    assert(sgsqlite_exec(second,test_pointer("INSERT INTO items VALUES(4,'busy',NULL)"))==-11);
    sgsqlite_tx_close(tx);sgsqlite_db_close(second);
    /* A statement keeps its connection alive after the public Db is dropped. */
    s=sgsqlite_prepare(db,test_pointer("SELECT name FROM items WHERE id=1"));assert(s>0);
    sgsqlite_db_close(db);assert(sgsqlite_live_connections()==1);assert(sgsqlite_step(s)==1);test_string(sgsqlite_column_text(s,0),"commit");sgsqlite_stmt_close(s);
    assert(sgsqlite_live_connections()==0);
    db=sgsqlite_open(test_pointer(path));assert(db>0);assert(count(db)==2);
    assert(sgsqlite_prepare(db,test_pointer("SELECT 1; SELECT 2"))==-2);
    assert(sgsqlite_prepare(db,test_pointer("SELECT load_extension('anything')"))<0);
    tx=sgsqlite_begin(db);s=sgsqlite_tx_prepare(tx,test_pointer("SELECT 1"));assert(s>0);sgsqlite_tx_close(tx);assert(sgsqlite_step(s)==-7);sgsqlite_stmt_close(s);
    sgsqlite_db_close(db);remove(path);
    assert(sgsqlite_live_connections()==0&&sgsqlite_live_statements()==0&&sgsqlite_live_transactions()==0);
    puts("sgsqlite native: passed");return 0;
}
