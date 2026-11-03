/* One translation unit fixes the bundled SQLite configuration for all hosts. */
#define SQLITE_THREADSAFE 1
#define SQLITE_OMIT_LOAD_EXTENSION 1
#define SQLITE_DQS 0
#define SQLITE_DEFAULT_FOREIGN_KEYS 1
#include "vendor/sqlite-amalgamation-3540100/sqlite3.c"

#include <limits.h>
extern long long sengoo_string_from_bytes_copy(long long,long long);
extern long long sengoo_ffi_buffer_from_bytes(long long,long long);

typedef struct SgDb {
    long long id,refs,tx;int exposed,internal;
    sqlite3 *db;struct SgDb *next;
} SgDb;
typedef struct SgStmt {
    long long id,tx;int row;SgDb *connection;sqlite3_stmt *stmt;
    struct SgStmt *next;
} SgStmt;
typedef struct SgTx {long long id;SgDb *connection;struct SgTx *next;} SgTx;
static SgDb *sq_dbs;
static SgStmt *sq_stmts;
static SgTx *sq_txs;
static long long sq_next=1,sq_status,sq_raw;
static char sq_message[512];
static void sq_clear(void) {sq_status=0;sq_raw=0;sq_message[0]=0;}
static long long sq_fail(long long status,long long raw,const char *message) {
    sq_status=status;sq_raw=raw;snprintf(sq_message,sizeof(sq_message),"%s",message?message:"");return -status;
}
static long long sq_identity(void) {
    if(sq_next==LLONG_MAX){sq_fail(13,SQLITE_FULL,"SQLite handle identities exhausted");return 0;}return sq_next++;
}
static long long sq_map(int rc) {
    switch(rc&255) {
        case SQLITE_NOMEM:return 14;
        case SQLITE_BUSY:case SQLITE_LOCKED:return 11;
        case SQLITE_NOTFOUND:return 5;
        case SQLITE_PERM:case SQLITE_AUTH:case SQLITE_READONLY:return 7;
        case SQLITE_TOOBIG:case SQLITE_RANGE:return 13;
        case SQLITE_MISUSE:case SQLITE_MISMATCH:case SQLITE_CONSTRAINT:return 2;
        case SQLITE_INTERRUPT:return 12;
        case SQLITE_CANTOPEN:case SQLITE_IOERR:case SQLITE_FULL:return 9;
        default:return 1;
    }
}
static int sq_control(SgDb *c,const char *sql) {
    c->internal=1;int rc=sqlite3_exec(c->db,sql,NULL,NULL,NULL);c->internal=0;return rc;
}
static void sq_rollback(SgDb *c) {if(c->tx){sq_control(c,"ROLLBACK");c->tx=0;}}
static long long sq_sql_error(SgDb *c,int rc) {
    int extended=c?sqlite3_extended_errcode(c->db):rc;
    if(extended==SQLITE_OK)extended=rc;
    char message[512];snprintf(message,sizeof(message),"%s",c?sqlite3_errmsg(c->db):sqlite3_errstr(rc));
    if(c)sq_rollback(c);
    return sq_fail(sq_map(rc),extended,message);
}
static SgDb *sq_db(long long id) {
    for(SgDb *c=sq_dbs;c;c=c->next)if(c->id==id&&c->exposed)return c;
    sq_fail(3,SQLITE_MISUSE,"SQLite connection is closed or invalid");return NULL;
}
static SgStmt *sq_stmt(long long id) {
    for(SgStmt *s=sq_stmts;s;s=s->next)if(s->id==id)return s;
    sq_fail(3,SQLITE_MISUSE,"SQLite statement is closed or invalid");return NULL;
}
static SgTx *sq_tx(long long id) {
    for(SgTx *t=sq_txs;t;t=t->next)if(t->id==id)return t;
    sq_fail(3,SQLITE_MISUSE,"SQLite transaction is closed or invalid");return NULL;
}
static int sq_access(SgDb *c,long long tx) {
    if(c->tx!=tx){sq_fail(7,SQLITE_MISUSE,"connection is reserved for its active transaction or transaction has ended");return 0;}return 1;
}
static void sq_release(SgDb *c) {
    if(--c->refs)return;
    SgDb **p=&sq_dbs;while(*p&&*p!=c)p=&(*p)->next;if(*p)*p=c->next;
    sqlite3_close_v2(c->db);free(c);
}
static int sq_authorize(void *context,int operation,const char *a,const char *b,const char *db,const char *trigger) {
    (void)a;(void)b;(void)db;(void)trigger;SgDb *c=(SgDb*)context;
    if((operation==SQLITE_TRANSACTION||operation==SQLITE_SAVEPOINT)&&!c->internal)return SQLITE_DENY;
    return SQLITE_OK;
}
long long sgsqlite_error_status(void) {return sq_status;}
long long sgsqlite_error_code(void) {return sq_raw;}
long long sgsqlite_error_message(void) {return sengoo_string_from_bytes_copy((long long)(intptr_t)sq_message,(long long)strlen(sq_message));}
long long sgsqlite_open(long long path) {
    sq_clear();if(!path||!*(const char*)(intptr_t)path)return sq_fail(2,SQLITE_MISUSE,"database path is empty");
    SgDb *c=(SgDb*)calloc(1,sizeof(*c));if(!c)return sq_fail(14,SQLITE_NOMEM,"connection allocation failed");
    int rc=sqlite3_open_v2((const char*)(intptr_t)path,&c->db,SQLITE_OPEN_READWRITE|SQLITE_OPEN_CREATE|SQLITE_OPEN_NOMUTEX,NULL);
    if(rc!=SQLITE_OK){long long r=sq_sql_error(c,rc);sqlite3_close(c->db);free(c);return r;}
    sqlite3_extended_result_codes(c->db,1);
    sqlite3_set_authorizer(c->db,sq_authorize,c);
    c->id=sq_identity();if(!c->id){sqlite3_close(c->db);free(c);return -13;}
    c->refs=1;c->exposed=1;c->next=sq_dbs;sq_dbs=c;return c->id;
}
long long sgsqlite_db_close(long long id) {
    SgDb *c=sq_db(id);if(!c)return -3;c->exposed=0;sq_release(c);return 0;
}
long long sgsqlite_busy_timeout(long long id,long long milliseconds) {
    sq_clear();SgDb *c=sq_db(id);if(!c)return -3;
    if(!sq_access(c,0))return -7;
    if(milliseconds<0||milliseconds>INT_MAX)return sq_fail(2,SQLITE_RANGE,"busy timeout must fit a nonnegative i32");
    int rc=sqlite3_busy_timeout(c->db,(int)milliseconds);return rc==SQLITE_OK?0:sq_sql_error(c,rc);
}
static long long sq_execute(SgDb *c,long long tx,long long sql) {
    if(!sq_access(c,tx))return -7;if(!sql)return sq_fail(2,SQLITE_MISUSE,"SQL text is null");
    int rc=sqlite3_exec(c->db,(const char*)(intptr_t)sql,NULL,NULL,NULL);
    return rc==SQLITE_OK?0:sq_sql_error(c,rc);
}
long long sgsqlite_exec(long long id,long long sql) {sq_clear();SgDb *c=sq_db(id);return c?sq_execute(c,0,sql):-3;}
static long long sq_prepare(SgDb *c,long long tx,long long sql) {
    if(!sq_access(c,tx))return -7;if(!sql)return sq_fail(2,SQLITE_MISUSE,"SQL text is null");
    sqlite3_stmt *stmt=NULL;const char *tail;
    int rc=sqlite3_prepare_v2(c->db,(const char*)(intptr_t)sql,-1,&stmt,&tail);
    if(rc!=SQLITE_OK)return sq_sql_error(c,rc);
    while(*tail==' '||*tail=='\t'||*tail=='\n'||*tail=='\r')++tail;
    if(!stmt||*tail){sqlite3_finalize(stmt);return sq_fail(2,SQLITE_MISUSE,"prepare requires exactly one SQL statement");}
    SgStmt *s=(SgStmt*)calloc(1,sizeof(*s));if(!s){sqlite3_finalize(stmt);return sq_fail(14,SQLITE_NOMEM,"statement allocation failed");}
    s->id=sq_identity();if(!s->id){sqlite3_finalize(stmt);free(s);return -13;}
    s->stmt=stmt;s->connection=c;s->tx=tx;++c->refs;s->next=sq_stmts;sq_stmts=s;return s->id;
}
long long sgsqlite_prepare(long long id,long long sql) {sq_clear();SgDb *c=sq_db(id);return c?sq_prepare(c,0,sql):-3;}
long long sgsqlite_stmt_close(long long id) {
    SgStmt **p=&sq_stmts;while(*p&&(*p)->id!=id)p=&(*p)->next;if(!*p)return -3;
    SgStmt *s=*p;*p=s->next;sqlite3_finalize(s->stmt);sq_release(s->connection);free(s);return 0;
}
static SgStmt *sq_binding(long long id,long long index) {
    sq_clear();SgStmt *s=sq_stmt(id);if(!s)return NULL;
    if(!sq_access(s->connection,s->tx))return NULL;
    if(index<=0||index>sqlite3_bind_parameter_count(s->stmt)){sq_fail(2,SQLITE_RANGE,"binding index is out of bounds (indices start at one)");return NULL;}
    if(s->row){sq_fail(2,SQLITE_MISUSE,"reset statement before rebinding");return NULL;}
    return s;
}
long long sgsqlite_bind_null(long long id,long long index) {
    SgStmt *s=sq_binding(id,index);if(!s)return -sq_status;
    int rc=sqlite3_bind_null(s->stmt,(int)index);return rc==SQLITE_OK?0:sq_sql_error(s->connection,rc);
}
long long sgsqlite_bind_i64(long long id,long long index,long long value) {
    SgStmt *s=sq_binding(id,index);if(!s)return -sq_status;
    int rc=sqlite3_bind_int64(s->stmt,(int)index,value);return rc==SQLITE_OK?0:sq_sql_error(s->connection,rc);
}
long long sgsqlite_bind_f64(long long id,long long index,double value) {
    SgStmt *s=sq_binding(id,index);if(!s)return -sq_status;
    int rc=sqlite3_bind_double(s->stmt,(int)index,value);return rc==SQLITE_OK?0:sq_sql_error(s->connection,rc);
}
long long sgsqlite_bind_bytes(long long id,long long index,long long bytes,long long length,long long text) {
    SgStmt *s=sq_binding(id,index);if(!s)return -sq_status;
    if(length<0||length>INT_MAX||(length&&!bytes))return sq_fail(2,SQLITE_RANGE,"invalid bound byte length");
    const char *p=bytes?(const char*)(intptr_t)bytes:"";
    int rc=text?sqlite3_bind_text(s->stmt,(int)index,p,(int)length,SQLITE_TRANSIENT):sqlite3_bind_blob(s->stmt,(int)index,p,(int)length,SQLITE_TRANSIENT);
    return rc==SQLITE_OK?0:sq_sql_error(s->connection,rc);
}
long long sgsqlite_step(long long id) {
    sq_clear();SgStmt *s=sq_stmt(id);if(!s)return -3;
    if(!sq_access(s->connection,s->tx))return -7;
    s->row=0;int rc=sqlite3_step(s->stmt);
    if(rc==SQLITE_ROW){s->row=1;return 1;}if(rc==SQLITE_DONE)return 0;
    return sq_sql_error(s->connection,rc);
}
long long sgsqlite_reset(long long id) {
    sq_clear();SgStmt *s=sq_stmt(id);if(!s)return -3;
    if(!sq_access(s->connection,s->tx))return -7;
    s->row=0;int rc=sqlite3_reset(s->stmt);sqlite3_clear_bindings(s->stmt);
    return rc==SQLITE_OK?0:sq_sql_error(s->connection,rc);
}
static SgStmt *sq_column(long long id,long long index) {
    sq_clear();SgStmt *s=sq_stmt(id);if(!s)return NULL;
    if(!sq_access(s->connection,s->tx))return NULL;
    if(!s->row){sq_fail(2,SQLITE_MISUSE,"column access requires a current row");return NULL;}
    if(index<0||index>=sqlite3_column_count(s->stmt)){sq_fail(2,SQLITE_RANGE,"column index is out of bounds (indices start at zero)");return NULL;}return s;
}
long long sgsqlite_column_type(long long id,long long index) {SgStmt *s=sq_column(id,index);return s?sqlite3_column_type(s->stmt,(int)index):-sq_status;}
long long sgsqlite_column_i64(long long id,long long index) {
    SgStmt *s=sq_column(id,index);if(!s)return 0;
    if(sqlite3_column_type(s->stmt,(int)index)!=SQLITE_INTEGER){sq_fail(2,SQLITE_MISMATCH,"column is not an integer");return 0;}return sqlite3_column_int64(s->stmt,(int)index);
}
double sgsqlite_column_f64(long long id,long long index) {
    SgStmt *s=sq_column(id,index);if(!s)return 0;
    int kind=sqlite3_column_type(s->stmt,(int)index);
    if(kind!=SQLITE_INTEGER&&kind!=SQLITE_FLOAT){sq_fail(2,SQLITE_MISMATCH,"column is not numeric");return 0;}return sqlite3_column_double(s->stmt,(int)index);
}
long long sgsqlite_column_text(long long id,long long index) {
    SgStmt *s=sq_column(id,index);if(!s)return -sq_status;
    if(sqlite3_column_type(s->stmt,(int)index)!=SQLITE_TEXT)return sq_fail(2,SQLITE_MISMATCH,"column is not text");
    const unsigned char *text=sqlite3_column_text(s->stmt,(int)index);int n=sqlite3_column_bytes(s->stmt,(int)index);
    if(!text)return sq_sql_error(s->connection,SQLITE_NOMEM);
    return sengoo_string_from_bytes_copy((long long)(intptr_t)text,n);
}
long long sgsqlite_column_blob(long long id,long long index) {
    SgStmt *s=sq_column(id,index);if(!s)return -sq_status;
    if(sqlite3_column_type(s->stmt,(int)index)!=SQLITE_BLOB)return sq_fail(2,SQLITE_MISMATCH,"column is not a blob");
    const void *bytes=sqlite3_column_blob(s->stmt,(int)index);int n=sqlite3_column_bytes(s->stmt,(int)index);
    return sengoo_ffi_buffer_from_bytes((long long)(intptr_t)bytes,n);
}
long long sgsqlite_begin(long long id) {
    sq_clear();SgDb *c=sq_db(id);if(!c)return -3;if(!sq_access(c,0))return -7;
    SgTx *t=(SgTx*)calloc(1,sizeof(*t));if(!t)return sq_fail(14,SQLITE_NOMEM,"transaction allocation failed");
    t->id=sq_identity();if(!t->id){free(t);return -13;}
    int rc=sq_control(c,"BEGIN");if(rc!=SQLITE_OK){free(t);return sq_sql_error(c,rc);}
    c->tx=t->id;t->connection=c;++c->refs;t->next=sq_txs;sq_txs=t;return t->id;
}
long long sgsqlite_tx_exec(long long id,long long sql) {sq_clear();SgTx *t=sq_tx(id);return t?sq_execute(t->connection,id,sql):-3;}
long long sgsqlite_tx_prepare(long long id,long long sql) {sq_clear();SgTx *t=sq_tx(id);return t?sq_prepare(t->connection,id,sql):-3;}
long long sgsqlite_commit(long long id) {
    sq_clear();SgTx *t=sq_tx(id);if(!t)return -3;SgDb *c=t->connection;if(!sq_access(c,id))return -7;
    int rc=sq_control(c,"COMMIT");if(rc!=SQLITE_OK)return sq_sql_error(c,rc);c->tx=0;return 0;
}
long long sgsqlite_rollback(long long id) {
    sq_clear();SgTx *t=sq_tx(id);if(!t)return -3;
    if(!sq_access(t->connection,id))return -7;
    int rc=sq_control(t->connection,"ROLLBACK");t->connection->tx=0;return rc==SQLITE_OK?0:sq_sql_error(t->connection,rc);
}
long long sgsqlite_tx_close(long long id) {
    SgTx **p=&sq_txs;while(*p&&(*p)->id!=id)p=&(*p)->next;if(!*p)return -3;
    SgTx *t=*p;*p=t->next;
    if(t->connection->tx==id)sq_rollback(t->connection);
    sq_release(t->connection);free(t);return 0;
}
long long sgsqlite_live_connections(void) {long long n=0;for(SgDb *c=sq_dbs;c;c=c->next)++n;return n;}
long long sgsqlite_live_statements(void) {long long n=0;for(SgStmt *s=sq_stmts;s;s=s->next)++n;return n;}
long long sgsqlite_live_transactions(void) {long long n=0;for(SgTx *t=sq_txs;t;t=t->next)++n;return n;}
long long sgsqlite_version(void) {return sengoo_string_from_bytes_copy((long long)(intptr_t)SQLITE_VERSION,(long long)strlen(SQLITE_VERSION));}
