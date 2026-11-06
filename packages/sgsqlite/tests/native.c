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
    assert(sgsqlite_changes(db)==1&&sgsqlite_error_status()==0);
    assert(sgsqlite_last_insert_rowid(db)==LLONG_MAX&&sgsqlite_error_status()==0);
    assert(sgsqlite_set_context(db,test_pointer("instance_id"),11,test_pointer("owner-世界"),12)==0);
    s=sgsqlite_prepare(db,test_pointer("SELECT sengoo_context('instance_id'),sengoo_context('missing')"));assert(s>0);
    assert(sgsqlite_step(s)==1);test_string(sgsqlite_column_text(s,0),"owner-世界");
    assert(sgsqlite_column_type(s,1)==SQLITE_NULL);sgsqlite_stmt_close(s);
    long long context_other=sgsqlite_open(test_pointer(":memory:"));assert(context_other>0);
    s=sgsqlite_prepare(context_other,test_pointer("SELECT sengoo_context('instance_id')"));assert(s>0);
    assert(sgsqlite_step(s)==1&&sgsqlite_column_type(s,0)==SQLITE_NULL);sgsqlite_stmt_close(s);sgsqlite_db_close(context_other);
    assert(sgsqlite_exec(db,test_pointer("CREATE TABLE guarded(value TEXT); CREATE TRIGGER owner_guard BEFORE INSERT ON guarded BEGIN SELECT CASE WHEN COALESCE(sengoo_context('instance_id'),'') != 'owner-世界' THEN RAISE(ABORT,'context mismatch') END; END;"))==0);
    assert(sgsqlite_exec(db,test_pointer("INSERT INTO guarded VALUES('allowed')"))==0);
    assert(sgsqlite_set_context(db,test_pointer("instance_id"),11,test_pointer("other"),5)==0);
    assert(sgsqlite_exec(db,test_pointer("INSERT INTO guarded VALUES('denied')"))==-2);
    assert(sgsqlite_set_context(db,test_pointer("instance_id"),11,test_pointer("owner-世界"),12)==0);
    const char malformed_context[]={'a',0,'b'};
    assert(sgsqlite_set_context(db,test_pointer("bad"),3,test_pointer(malformed_context),3)==-20);
    assert(sgsqlite_set_context(db,test_pointer(""),0,test_pointer("value"),5)==-2);
    char context_key[32];
    for(int i=0;i<63;i++){snprintf(context_key,sizeof(context_key),"bounded-key-%d",i);assert(sgsqlite_set_context(db,test_pointer(context_key),(long long)strlen(context_key),test_pointer("value"),5)==0);}
    assert(sgsqlite_set_context(db,test_pointer("one-too-many"),12,test_pointer("value"),5)==-13);
    for(int i=0;i<1000;i++)assert(sgsqlite_set_context(db,test_pointer("bounded-key-0"),13,test_pointer("replacement"),11)==0);
    s=sgsqlite_prepare(db,test_pointer("SELECT id,name,bytes,NULL FROM items"));assert(s>0);assert(sgsqlite_step(s)==1);
    assert(sgsqlite_column_i64(s,0)==LLONG_MAX);
    test_string(sgsqlite_column_text(s,1),name);
    long long blob=sgsqlite_column_blob(s,2);TestBytes *b=(TestBytes*)(intptr_t)blob;assert(b->size==3&&!memcmp(b->data,bytes,3));
    assert(sgsqlite_column_type(s,3)==SQLITE_NULL);
    long long copied=sgsqlite_column_text(s,1);assert(sgsqlite_step(s)==0);test_string(copied,name);test_buffer_free(blob);sgsqlite_stmt_close(s);
    assert(sgsqlite_exec(db,test_pointer("BEGIN"))==-7);
    long long tx=sgsqlite_begin(db);assert(tx>0);assert(sgsqlite_begin(db)==-7);
    assert(sgsqlite_set_context(db,test_pointer("instance_id"),11,test_pointer("cannot-change-in-tx"),19)==-7);
    assert(sgsqlite_backup_to(db,test_pointer("unused-backup.db"))==-7);
    sgsqlite_changes(db);assert(sgsqlite_error_status()==7);
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
    const char *backup_path="sgsqlite-backup.db";remove(backup_path);
    assert(sgsqlite_backup_to(db,test_pointer(path))==-2);
    assert(sgsqlite_backup_to(db,test_pointer(":memory:"))==-2);
    assert(sgsqlite_backup_to(db,test_pointer(backup_path))==0);
    long long restored=sgsqlite_open(test_pointer(backup_path));assert(restored>0&&count(restored)==2);
    s=sgsqlite_prepare(restored,test_pointer("SELECT sengoo_context('instance_id')"));assert(s>0);
    assert(sgsqlite_step(s)==1&&sgsqlite_column_type(s,0)==SQLITE_NULL);sgsqlite_stmt_close(s);sgsqlite_db_close(restored);remove(backup_path);
    /* A statement keeps its connection alive after the public Db is dropped. */
    s=sgsqlite_prepare(db,test_pointer("SELECT name,sengoo_context('instance_id') FROM items WHERE id=1"));assert(s>0);
    sgsqlite_db_close(db);assert(sgsqlite_live_connections()==1);assert(sgsqlite_step(s)==1);test_string(sgsqlite_column_text(s,0),"commit");test_string(sgsqlite_column_text(s,1),"owner-世界");sgsqlite_stmt_close(s);
    assert(sgsqlite_live_connections()==0);
    db=sgsqlite_open(test_pointer(path));assert(db>0);assert(count(db)==2);
    assert(sgsqlite_prepare(db,test_pointer("SELECT 1; SELECT 2"))==-2);
    assert(sgsqlite_prepare(db,test_pointer("SELECT load_extension('anything')"))<0);
    tx=sgsqlite_begin(db);s=sgsqlite_tx_prepare(tx,test_pointer("SELECT 1"));assert(s>0);sgsqlite_tx_close(tx);assert(sgsqlite_step(s)==-7);sgsqlite_stmt_close(s);
    sgsqlite_db_close(db);remove(path);
    assert(sgsqlite_live_connections()==0&&sgsqlite_live_statements()==0&&sgsqlite_live_transactions()==0);
    puts("sgsqlite native: passed");return 0;
}
