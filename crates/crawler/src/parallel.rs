use std::{sync::mpsc::{Receiver, Sender, self, TryRecvError}, collections::HashSet, thread::{JoinHandle, self}};

use reqwest::Url;

use crate::{get::GetHandle, Error, url::{get_links_from_html, write_to_file}};


pub(crate) enum ReqMsg {
    FETCH(Url),
    READY,
    EXIT,
}

pub(crate) enum RspMsg {
    READY,
    RESPOND(HashSet<Url>, Option<Error>)
}

pub struct FetchPool {
    t_count: usize,
    ft_handles: Vec<FetchThreadHandle>,
    result_set: HashSet<Url>,
    error_vec: Vec<Error>
}

impl FetchPool {
    pub(crate) fn new(t_count: usize) -> Self {

        let mut ft_handles = vec![];

        // spawn threads
        for _ in 0..t_count {

            ft_handles.push(
                FetchThread::spawn()
            );

        }

        Self {
            t_count,
            ft_handles,
            result_set: HashSet::new(),
            error_vec: vec![]
        }

    }

    #[allow(dead_code)]
    pub(crate) fn t_count(&self) -> usize {
        self.t_count
    }

    pub(crate) fn fetch_single(&mut self, url: &Url) {
        if let Some(ft_handle) = self.ft_handles.iter().next() {
            match ft_handle.recv() {
                RspMsg::RESPOND(urls, e) => {
                    self.result_set.extend(urls);
                    if let Some(e) = e {
                        self.error_vec.push(e);
                    }
                },
                RspMsg::READY => {
                    ft_handle.fetch(&url);
                }
            }
        }
    }

    pub(crate) fn fetch(&mut self, new_urls: HashSet<Url>) {
        let mut ft_cycler = self.ft_handles.iter().cycle();
        for url in new_urls.into_iter() {
            while let Some(ft_handle) = ft_cycler.next() {
                match ft_handle.try_recv() {
                    Ok(msg) => {
                        match msg {
                            RspMsg::RESPOND(urls, e) => {
                                self.result_set.extend(urls);
                                if let Some(e) = e {
                                    self.error_vec.push(e);
                                }
                            },
                            RspMsg::READY => {
                                ft_handle.fetch(&url);
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        match e {
                            mpsc::TryRecvError::Empty => continue,
                            mpsc::TryRecvError::Disconnected => panic!("{}", e)
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn get_results(&mut self) -> (HashSet<Url>, String) {

        for ft_handle in self.ft_handles.iter() {
            match ft_handle.recv() {
                RspMsg::READY => ft_handle.ready(),

                RspMsg::RESPOND(urls, e) => {
                    self.result_set.extend(urls);
                    if let Some(e) = e {
                        self.error_vec.push(e);
                    }
                }
            }
        }


        let err_msgs = format!(
            "Errors: {:#?}", self.error_vec 
        );
        self.error_vec.clear();

        let res = self.result_set.clone();
        self.result_set.clear();

        (res, err_msgs)
    }

    pub(crate) fn close(mut self) {
        let vc = vec![];
        let fths = self.ft_handles;
        self.ft_handles = vc;
        for ft_handle in fths.into_iter() {
            ft_handle.kill()
        }
    }
    

}

struct FetchThread;

impl FetchThread {
    fn spawn() -> FetchThreadHandle {
        let get_handle = GetHandle::new();
        let (to_thread, from_master) = mpsc::channel();
        let (to_master, from_thread) = mpsc::channel();

        let mth = MasterThreadHandle {
            to_master,
            from_master
        };

        let handle = thread::spawn(move || {

            loop {
                // to_master.send(RspMsg::READY).unwrap();
                mth.ready();
                
                match mth.receive() {

                    ReqMsg::EXIT => break,

                    ReqMsg::READY => continue,

                    ReqMsg::FETCH(url) => {
                        
                        // fetch data from url
                        let fetch = get_handle.get_url(url);
                        if fetch.is_err() {
                            mth.send_err(fetch.unwrap_err());
                            continue;
                        }

                        let fetch = fetch.unwrap();

                        if fetch.is_html() {
                            let found_urls = get_links_from_html(&fetch);
                            println!("Visited: {} found {} links", fetch.get_url(), found_urls.len());
                            mth.send_urls(found_urls);
                        }

                        let write_res = write_to_file(fetch);
                        // if write failes: send error and continue
                        if write_res.is_err() {
                            mth.send_err(
                                Error::from(write_res.unwrap_err())
                            )
                        }
                    }
                }
            }
        });

        FetchThreadHandle {
            join_handle: handle,
            to_thread,
            from_thread
        }
    }

}

struct MasterThreadHandle {
    to_master: Sender<RspMsg>,
    from_master: Receiver<ReqMsg>
}

impl MasterThreadHandle {
    fn ready(&self) {
        self.to_master.send(RspMsg::READY).unwrap()
    }

    fn send_urls(&self, found_urls: HashSet<Url>) {
        self.to_master.send(RspMsg::RESPOND(found_urls, None)).unwrap();
    }

    fn send_err(&self, e: Error) {
        self.to_master.send(RspMsg::RESPOND(HashSet::new(), Some(e))).unwrap();
    }

    fn receive(&self) -> ReqMsg {
        self.from_master.recv().unwrap()
    }
}

struct FetchThreadHandle {
    join_handle: JoinHandle<()>,
    to_thread: Sender<ReqMsg>,
    from_thread: Receiver<RspMsg>
}

impl FetchThreadHandle {
    fn kill(self) {
        self.to_thread.send(ReqMsg::EXIT).unwrap();
        self.join_handle.join().unwrap();
    }

    fn ready(&self) {
        self.to_thread.send(ReqMsg::READY).unwrap()
    }

    fn fetch(&self, url: &Url) {
        self.to_thread.send(ReqMsg::FETCH(url.to_owned())).unwrap();
    }

    fn recv(&self) -> RspMsg {
        self.from_thread.recv().unwrap()
    }

    fn try_recv(&self) -> Result<RspMsg, TryRecvError> {
        self.from_thread.try_recv()
    }
}

