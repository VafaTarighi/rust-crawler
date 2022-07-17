use std::{collections::HashSet, process, os::unix::net::UnixStream, io::{Read, Write}, time::Duration};

use nix::{unistd::{fork, ForkResult, Pid}, sys::wait::waitpid};
use reqwest::Url;

use polling::{Event, Poller};


use crate::{get::GetHandle, Error, url::{get_links_from_html, write_to_file}};

const MSG_CODE_LEN: usize = 8; // code padding is 9 bytes

 // for Urls stream is specific number of bytes, for other messages is 1 bytes

pub(crate) enum ReqMsg {
    FETCH(Url),           // "FFFF_FFFF" -> "F"
    IDLE,
    EXIT,                 // "FFFF_FFFF" -> "E"
}

pub(crate) enum RspMsg {
    PAGE(HashSet<Url>),   // "FFFF_FFFF" -> "P"
    START,
    FIN,                  // "FFFF_FFFF" -> "I"
}

pub struct FetchPool {
    t_count: usize,
    ft_handles: Vec<FetchProcessHandle>,
    work: usize,
    results: Vec<HashSet<Url>>,
    poller: Poller,
}

impl FetchPool {
    pub(crate) fn new(t_count: usize) -> Self {

        let poller = Poller::new().unwrap();
        let mut ft_handles = vec![];

        // spawn threads
        for i in 0..t_count {
            let fth = FetchProcess::spawn();
            poller.add(&fth.child, Event::readable(i)).unwrap();
            ft_handles.push(fth);
        }
        println!("Child Spawning Finished");
        Self {
            t_count,
            ft_handles,
            work: 0,
            results: vec![],
            poller,
        }

    }

    #[allow(dead_code)]
    pub(crate) fn t_count(&self) -> usize {
        self.t_count
    }

    pub(crate) fn fetch(&mut self, page: HashSet<Url>) {
        self.work += page.len();
        let mut events = vec![];
        let mut url_itr = page.into_iter();
        let mut url = url_itr.next();
        loop {
            if url == None {
                break;
            }

            events.clear();
            self.poller.wait(&mut events, None).unwrap();

            for ev in &events {
                let fth = &mut self.ft_handles[ev.key];
                match fth.recv() {
                    Some(RspMsg::PAGE(page)) => {
                        self.results.push(page);
                        fth.send_url(url.unwrap());
                        url = url_itr.next();
                    },
                    Some(RspMsg::FIN) => {
                        fth.send_url(url.clone().unwrap());
                        self.work -= 1;
                        url = url_itr.next();
                    },
                    Some(RspMsg::START) => {
                        fth.send_url(url.clone().unwrap());
                        url = url_itr.next();
                    }
                    None => ()
                }

                self.poller.modify(&fth.child, Event::readable(ev.key)).unwrap();

                
                if url == None {
                    break;
                }
            }

        }
    }

    pub(crate) fn get_current(&mut self, timeout: Option<Duration>) -> Option<Vec<HashSet<Url>>> {
        if self.work == 0 {
            return None
        }

        let mut events = vec![];
        events.clear();
        self.poller.wait(&mut events, timeout).unwrap();
        for ev in &events {
            let fth = &mut self.ft_handles[ev.key];
            match fth.recv() {
                Some(RspMsg::PAGE(page)) => self.results.push(page),
                Some(RspMsg::FIN) => self.work -= 1,
                Some(RspMsg::START) => (),
                None => ()
            }

            fth.idle();

            self.poller.modify(&fth.child, Event::readable(ev.key)).unwrap();
        }

        let pages = self.results.to_owned();
        self.work -= pages.len();
        self.results.clear();
        
        Some(pages)
    }

    pub(crate) fn get_wait(&mut self) -> Option<Vec<HashSet<Url>>> {
        if self.work == 0 {
            return None
        }

        for fth in &mut self.ft_handles {
            match fth.recv() {
                Some(RspMsg::PAGE(page)) => self.results.push(page),
                Some(RspMsg::FIN) => self.work -= 1,
                Some(RspMsg::START) => (),
                None => ()
            }

            fth.idle();

        }

        let pages = self.results.to_owned();
        self.work -= pages.len();
        self.results.clear();
        
        Some(pages)
    }

    pub(crate) fn close(mut self) {

        let vc = vec![];
        let fths = self.ft_handles;
        self.ft_handles = vc;
        fths.into_iter()
            .for_each(|ft_handle| {
                ft_handle.kill()
            })
    }
    

}

struct FetchProcess;

impl FetchProcess {
    fn spawn() -> FetchProcessHandle {
        let (child, parent) = UnixStream::pair().unwrap();


        let mut mth = MasterProcessHandle {
            parent
        };

        

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                let get_handle = GetHandle::new();
                
                mth.strt();
                
                loop {
                    match mth.receive() {
    
                        ReqMsg::EXIT => break,

                        ReqMsg::IDLE => mth.strt(),
    
                        ReqMsg::FETCH(url) => {
                            
                            // fetch data from url
                            let fetch = get_handle.get_url(url);
                            if fetch.is_err() {
                                mth.print_err(fetch.unwrap_err());
                                mth.fin();
                                continue;
                            }
    
                            let fetch = fetch.unwrap();
    
                            if fetch.is_html() {
                                let found_urls = get_links_from_html(&fetch);
                                println!("Visited: {} found {} links", fetch.get_url(), found_urls.len());
                                mth.send_urls(found_urls);
                            } else {
                                mth.fin();
                            }
    
                            let write_res = write_to_file(fetch);
                            // if write failes: send error and continue
                            if write_res.is_err() {
                                mth.print_err(
                                    write_res.unwrap_err()
                                )
                            }
                        }
                    }
                }

                mth.close();
                process::exit(0);

            },
            Ok(ForkResult::Parent { child: cpid }) => {
                // child.set_nonblocking(true).unwrap();
                
                return FetchProcessHandle {
                    cpid,
                    child
                }
            },
            Err(errno) => {
                panic!("{}", errno);
            }
        }
    }

}


struct FetchProcessHandle {
    cpid: Pid,
    child: UnixStream
}

impl FetchProcessHandle {


    fn send_url(&mut self, url: Url) {
        write!(self.child, "F{:0width$}{}", url.as_str().len(), url.as_str(), width = MSG_CODE_LEN).unwrap();
        // self.child.flush().unwrap();
    }

    fn recv(&mut self) -> Option<RspMsg> {
        let mut code = vec![0];
        self.child.read_exact(code.as_mut_slice()).unwrap();

        let code = String::from_utf8(code).unwrap();

        match code.as_str() {
            "I" => Some(RspMsg::FIN),

            "S" => Some(RspMsg::START),

            "P" => {
                let mut length = vec![0; MSG_CODE_LEN];
                self.child.read_exact(length.as_mut_slice()).unwrap();
                let length = String::from_utf8(length).unwrap();
                let len = length.parse().unwrap();

                let mut urls = vec![0; len];
                self.child.read_exact(urls.as_mut_slice()).unwrap();
                let urls = String::from_utf8(urls).unwrap();

                Some(RspMsg::PAGE(
                                    urls.split_terminator('\n')
                                        .map(Url::parse)
                                        .map(Result::unwrap)
                                        .collect()
                                    ))
            },
            _ => {
                println!("Unknown code received to master: {}", code);
                None
            }
        }
    }

    fn idle(&mut self) {
        write!(self.child, "D").unwrap();
    }


    fn kill(mut self) {
        self.child.write_all(b"E").unwrap();
        self.child.flush().unwrap();
        waitpid(self.cpid, None).unwrap();
        self.child.shutdown(std::net::Shutdown::Both).unwrap();
    }
}


struct MasterProcessHandle{
    parent: UnixStream
}

impl MasterProcessHandle {

    fn close(&mut self) {
        // self.parent.flush().unwrap();
        self.parent.shutdown(std::net::Shutdown::Both).unwrap();
    }

    fn send_urls(&mut self, found_urls: HashSet<Url>) {
        let urls =  found_urls.into_iter()
            .fold(String::new(), |acc, x| {
                acc + x.as_str() + "\n"
            });

        let len = urls.len();

        write!(self.parent, "P{:0width$}{}", len, urls, width = MSG_CODE_LEN).unwrap();
        self.parent.flush().unwrap();
    }

    fn strt(&mut self) {
        self.parent.write(b"S").unwrap();
    }

    fn fin(&mut self) {
        self.parent.write(b"I").unwrap();
        // self.parent.flush().unwrap();
    }

    fn print_err(&mut self, e: Error) {
        println!("{}", e);
    }

    fn receive(&mut self) -> ReqMsg {
        let mut code = vec![0];
        self.parent.read_exact(code.as_mut_slice()).unwrap();
        let code = String::from_utf8(code).unwrap();

        match code.as_str() {


            "F" => {
                let mut length = vec![0; MSG_CODE_LEN];
                self.parent.read_exact(length.as_mut_slice()).unwrap();
                let length = String::from_utf8(length).unwrap();

                let len = length.parse().unwrap();

                let mut url = vec![0; len];
                self.parent.read_exact(url.as_mut_slice()).unwrap();
                let url = String::from_utf8(url).unwrap();

                ReqMsg::FETCH(Url::parse(&url).unwrap())
            },

            "D" => ReqMsg::IDLE,


            "E" => ReqMsg::EXIT,


            _ => panic!("Unknown code recieved to worker: {}", code)
        }
    }
}
