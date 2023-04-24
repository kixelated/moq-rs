const DB_VERSION = 8;
let db: IDBDatabase;
let status: 'none' | 'pending' | 'inited' = 'none';

const getStatus = () => {
    return status;
};

const init = (): Promise<boolean> => {
    return new Promise((resolve, reject) => {
        try {
            if (status !== 'none') {
                resolve(false);
            }
            status = 'pending';

            console.log('opening db');
            const DBOpenRequest = window.indexedDB.open('logs', DB_VERSION);

            console.log('opened db');

            // Register two event handlers to act on the database being opened successfully, or not
            DBOpenRequest.onerror = (event) => {
                console.error('error opening database', event);
                reject('error opening database');
            };

            DBOpenRequest.onsuccess = (event) => {
                console.log('Database initialised.');
                db = DBOpenRequest.result;
                status = 'inited';
                resolve(true);
            };

            DBOpenRequest.onupgradeneeded = (event: any) => {
                console.log('onupgradeneeded', event)

                db = event.target.result;

                db.onerror = (event) => {
                    console.error('error loading database', event);
                    reject('error loading database');
                };

                // Create an objectStore for this database
                try {
                    let testStore, logStore, resultStore;
                    if (db.objectStoreNames.contains('tests')) {
                        db.deleteObjectStore('tests')
                    }

                    if (db.objectStoreNames.contains('test_logs')) {
                        db.deleteObjectStore('test_logs')
                    }

                    if (db.objectStoreNames.contains('test_results')) {
                        db.deleteObjectStore('test_results')
                    }


                    logStore = db.createObjectStore('test_logs', { keyPath: 'key', autoIncrement: true });
                    testStore = db.createObjectStore('tests', { keyPath: 'testId' });
                    resultStore = db.createObjectStore('test_results', { keyPath: 'key', autoIncrement: true });

                    testStore.createIndex('ix_testId', 'testId', { unique: false });
                    logStore.createIndex('ix_testId', 'testId', { unique: false });
                    logStore.createIndex('ix_chunk_no', 'no', { unique: false });
                    resultStore.createIndex('ix_testId', 'testId', { unique: false });

                    console.log('Object stores created.');
                    status = 'inited';
                } catch (ex) {
                    console.error('exception in onupgradeneeded', ex);
                    reject(ex);
                }
                // a 1 sec delay to avoid the error "A version change transaction is running"
                setTimeout(() => resolve(true), 1000);
            };
        } catch (ex) {
            console.error('Error in opening db', ex);
            reject(ex);
        }
    });
};

const addTestEntry = (test: any) => {
    addEntry('tests', test);
};

const addLogEntry = (log: any) => {
    addEntry('test_logs', log);
};

const addResultEntry = (result: any) => {
    addEntry('test_results', result);
};

const addEntry = (storeName: string, entry: any) => {
    // console.log('addEntry to %s', storeName, entry);

    const transaction = db.transaction([storeName], 'readwrite');
    transaction.oncomplete = () => {
        // console.log('added to %s', storeName);
    };
    transaction.onerror = () => {
        console.error('add to %s | error: %s', storeName, transaction.error);
    };
    const objectStore = transaction.objectStore(storeName);
    const objectStoreRequest = objectStore.add(entry);
    objectStoreRequest.onsuccess = (event) => {
        // console.log('request successful - %s', storeName)
    };
};

const getLogs = async (testId?: string): Promise<any[]> => {
    if (testId) {
        return getEntriesByTestId('test_logs', testId);
    } else {
        return getEntries('test_logs');
    }
    
};

const getResults = async (testId: string): Promise<any[]> => {
    return getEntriesByTestId('test_results', testId);
};

const getTests = async (): Promise<any[]> => {
    return getEntries('tests');
};

const getEntries = async (storeName: string): Promise<any[]> => {
    console.log('in getEntries | %s', storeName);
    return new Promise((resolve, reject) => {
        const transaction = db.transaction([storeName], 'readonly');
        transaction.oncomplete = () => {
            console.log('getEntries | transaction complete: %s', storeName);
            resolve(objectStoreRequest.result);
        };
        transaction.onerror = () => {
            console.error('add to %s | error: %s', storeName, transaction.error);
            reject('transaction error ' + transaction.error);
        };
        const objectStore = transaction.objectStore(storeName);
        const objectStoreRequest = objectStore.getAll();
        objectStoreRequest.onsuccess = (event) => {
            console.log('request successful', event);
        };
    });
};

const getEntriesByTestId = async (storeName: string, testId: string): Promise<any[]> => {
    console.log('in getEntries | store: %s testId: %s', storeName, testId);
    return new Promise((resolve, reject) => {
        const keyRangeValue = IDBKeyRange.only(testId);
        const transaction = db.transaction([storeName], 'readonly');
        transaction.oncomplete = () => {
            console.log('transaction complete', objectStoreRequest.result);
            resolve(objectStoreRequest.result);
        };
        transaction.onerror = () => {
            console.error('add to %s | error: %s', storeName, transaction.error);
            reject('transaction error ' + transaction.error);
        };
        const objectStore = transaction.objectStore(storeName);
        const storeIndex = objectStore.index("ix_testId");
        const objectStoreRequest = storeIndex.getAll(keyRangeValue);
        objectStoreRequest.onsuccess = (event) => {
            console.log('request successful', event);
        };
    });
};

const getDb = () => {
    return db;
};


export const dbStore = {
    getStatus,
    addTestEntry,
    addLogEntry,
    addResultEntry,
    init,
    getLogs,
    getResults,
    getDb,
    getTests
};
